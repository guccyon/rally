//! 本部が生徒 1 人の利用の様子を見る（admin-web の `GET /students/{student_id}/activity`）ことを、保存先を Postgres にした
//! 組み立て（`server::router_with_postgres`）で通す。生徒が student-app の操作で予約し・キャンセルし・動画添削を受けて
//! 完了を記録したことが、これからの予約・キャンセルの件数・直近の受けたレッスンに出ること、送った動画の数と返却待ちの数が
//! 本部の返却待ちの一覧（`GET /video-reviews/pending`）と同じ判定で数えられること、返信のないまま既定の時間の過ぎた
//! スレッドの数を messaging の保存先から読み、`listUnansweredMessageThreads` に `student_id` だけを付けた一覧と同じ数に
//! なること、その生徒がつけた直近の低い評価の件数と未対応の件数を評価の保存先から読み、`listLowRatedLessonReviews` に
//! `student_id` だけ（と `handled=false`）を付けた一覧と同じ数になること（本部が対応済みの印を付けると未対応から外れること・
//! 照会した時刻で変わることを含む）、何も保存しないこと、
//! コーチ・生徒は 403、生徒でない ID は 404 であることを確かめる。契約の形の細かい検査は interface の
//! `tests/student_activity.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test student_activity -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    identity::{
        RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer, account_id_uuid,
    },
    in_memory::{ManualClock, RecordingAnalyticsSink},
    messaging::MessageThreadRepository,
    review::{LessonReviewRepository, lesson_review_id_uuid},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    identity::{AccountId, Role},
    lesson::LessonCompletionId,
    messaging::{MessageBody, MessageContent, MessageThread},
    review::{LessonReview, LessonReviewComment, Rating},
};
use infra::{
    MockTokenService, PgAccountRepository, PgLessonReviewRepository, PgMessageThreadRepository,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

/// 2026-10-05T00:00:00Z から `secs` 秒後（Postgres の時刻はマイクロ秒までなので、秒単位にする）。
fn at(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_791_158_400 + secs)
}

async fn send(
    router: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let body = body.map_or_else(Body::empty, |b| Body::from(b.to_string()));
    let response = router
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

/// `token` で `path` に POST し、`expected` の status であることを確かめて本文を返す。
async fn post_ok(
    router: &Router,
    path: &str,
    token: &str,
    body: Value,
    expected: StatusCode,
) -> Value {
    let (status, body) = send(router, "POST", path, Some(token), Some(body)).await;
    assert_eq!(status, expected, "{path}: {body}");
    body
}

/// 役割 `role` のアカウントをユースケースを直に呼んで登録し（契約にコーチ・本部の登録の操作がない）、ID とトークンを返す。
async fn register(
    pool: &PgPool,
    email: &str,
    display_name: &str,
    role: Role,
    sports: Vec<Sport>,
) -> (String, String) {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        ManualClock::new(at(0)),
    )
    .execute(RegisterAccountCommand {
        email: email.to_string(),
        display_name: display_name.to_string(),
        role,
        sports,
    })
    .await
    .unwrap();
    let token = MockTokenService
        .issue(TokenClaims {
            account_id: account.id(),
            role,
        })
        .into_string();
    (account_id_uuid::format(account.id()), token)
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_sees_upcoming_bookings_cancellations_lessons_and_videos_of_a_student(
    pool: PgPool,
) {
    let (coach_id, coach) = register(
        &pool,
        "coach@example.com",
        "たなか",
        Role::Coach,
        vec![Sport::Tennis],
    )
    .await;
    let (headquarters_id, headquarters) =
        register(&pool, "hq@example.com", "本部", Role::Headquarters, vec![]).await;
    let clock = ManualClock::new(at(60));
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    let (status, account) = send(
        &router,
        "POST",
        "/accounts",
        None,
        Some(json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "tennis" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{account}");
    let student_id = account["student"]["student_id"]
        .as_str()
        .unwrap()
        .to_string();
    let student = account["access_token"].as_str().unwrap().to_string();

    // 2 つの枠を予約し、遅いほうをキャンセルする
    let mut booking_ids = Vec::new();
    for day in ["2099-01-01", "2099-01-02"] {
        let slot = post_ok(
            &router,
            "/slots",
            &coach,
            json!({
                "sport": "tennis",
                "starts_at": format!("{day}T10:00:00Z"),
                "ends_at": format!("{day}T11:00:00Z"),
            }),
            StatusCode::CREATED,
        )
        .await;
        let booking = post_ok(
            &router,
            "/bookings",
            &student,
            json!({ "slot_id": slot["slot_id"], "is_trial": false }),
            StatusCode::CREATED,
        )
        .await;
        booking_ids.push(booking["booking_id"].as_str().unwrap().to_string());
    }
    clock.set(at(120));
    post_ok(
        &router,
        &format!("/bookings/{}/cancel", booking_ids[1]),
        &student,
        json!({}),
        StatusCode::OK,
    )
    .await;

    // 動画を送り、添削が返されてから完了を記録する
    let video = post_ok(
        &router,
        "/videos",
        &student,
        json!({ "coach_id": coach_id, "sport": "tennis", "content_type": "video/mp4" }),
        StatusCode::CREATED,
    )
    .await;
    let video_id = video["video_id"].as_str().unwrap();
    clock.set(at(180));
    post_ok(
        &router,
        &format!("/videos/{video_id}/review"),
        &coach,
        json!({ "comment": "フォロースルーを大きく" }),
        StatusCode::OK,
    )
    .await;
    clock.set(at(240));
    let lesson = post_ok(
        &router,
        "/lessons/completions",
        &student,
        json!({ "lesson_type": "async_lesson", "video_id": video_id }),
        StatusCode::CREATED,
    )
    .await;

    // もう 2 本送り、どちらも返却を待っている
    for _ in 0..2 {
        post_ok(
            &router,
            "/videos",
            &student,
            json!({ "coach_id": coach_id, "sport": "tennis", "content_type": "video/mp4" }),
            StatusCode::CREATED,
        )
        .await;
    }

    let path = format!("/students/{student_id}/activity");
    let (status, body) = send(&router, "GET", &path, Some(&headquarters), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["student_id"], student_id);
    assert_eq!(body["display_name"], "たろう");
    assert_eq!(body["cancelled_booking_count"], 1);
    assert_eq!(body["submitted_video_count"], 3, "返却済みも数える");
    assert_eq!(body["pending_video_review_count"], 2);
    let (status, pending) = send(
        &router,
        "GET",
        "/video-reviews/pending",
        Some(&headquarters),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{pending}");
    assert_eq!(
        pending["videos"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|video| video["student_id"] == student_id)
            .count(),
        2,
        "返却待ちの一覧に載るこの生徒の動画の数と同じ"
    );
    let upcoming = body["upcoming_bookings"].as_array().unwrap();
    assert_eq!(upcoming.len(), 1, "キャンセルした予約は載せない: {body}");
    assert_eq!(upcoming[0]["booking"]["booking_id"], booking_ids[0]);
    assert_eq!(upcoming[0]["booking"]["status"], "pending");
    assert_eq!(upcoming[0]["coach_display_name"], "たなか");
    assert_eq!(
        body["recent_lessons"],
        json!([{
            "lesson_id": lesson["lesson_id"],
            "coach_id": coach_id,
            "coach_display_name": "たなか",
            "lesson_type": "async_lesson",
            "completed_at": "2026-10-05T00:04:00Z",
        }])
    );

    // 照会だけで何も変えない（2 回読んでも同じ）
    let (_, again) = send(&router, "GET", &path, Some(&headquarters), None).await;
    assert_eq!(again, body);

    for token in [&coach, &student] {
        let (status, body) = send(&router, "GET", &path, Some(token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
    for id in [&coach_id, &headquarters_id] {
        let (status, body) = send(
            &router,
            "GET",
            &format!("/students/{id}/activity"),
            Some(&headquarters),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "生徒でない: {body}");
    }
}

/// 生徒 `student` とコーチ `coach` のスレッドを `opened_at` に開いて保存先に書き、`exchange` の順に（生徒が送ったか, 送信時刻）で
/// 本文を送る。
async fn thread(
    threads: &PgMessageThreadRepository,
    student: AccountId,
    coach: AccountId,
    opened_at: SystemTime,
    exchange: &[(bool, SystemTime)],
) {
    let thread = threads
        .open(MessageThread::open(threads.next_thread_id(), student, coach, opened_at).unwrap())
        .await
        .unwrap();
    for &(by_student, at) in exchange {
        let message_id = threads.next_message_id();
        threads
            .update(thread.id(), |t| {
                t.send(
                    message_id,
                    if by_student { student } else { coach },
                    MessageContent::Text(MessageBody::new("本部には見せない本文").unwrap()),
                    true,
                    at,
                )
                .map(|_| ())
            })
            .await
            .unwrap();
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn counts_unanswered_message_threads_of_the_student_as_the_default_list_from_postgres(
    pool: PgPool,
) {
    let mut coaches = Vec::new();
    for (email, name) in [
        ("tanaka@example.com", "たなか"),
        ("ito@example.com", "いとう"),
        ("sato@example.com", "さとう"),
        ("kato@example.com", "かとう"),
    ] {
        let (coach_id, _) = register(&pool, email, name, Role::Coach, vec![Sport::Tennis]).await;
        coaches.push(account_id_uuid::parse(&coach_id).unwrap());
    }
    let (_, headquarters) =
        register(&pool, "hq@example.com", "本部", Role::Headquarters, vec![]).await;
    let mut students = Vec::new();
    for (email, name) in [
        ("taro@example.com", "たろう"),
        ("jiro@example.com", "じろう"),
    ] {
        let (student_id, _) =
            register(&pool, email, name, Role::Student, vec![Sport::Tennis]).await;
        students.push(student_id);
    }
    let taro = account_id_uuid::parse(&students[0]).unwrap();
    let jiro = account_id_uuid::parse(&students[1]).unwrap();
    let start = at(30 * 24 * 3600);
    let hour = Duration::from_secs(3600);
    let opened_at = at(0);
    let threads = PgMessageThreadRepository::new(pool.clone());
    // たろうのスレッドは、照会の 24 時間ちょうど前に送った 1 件（たなか）・23 時間前に送った 1 件（いとう）・
    // コーチが返した 1 件（さとう）・発言のない 1 件（かとう）。じろうのスレッドは 72 時間前に送った 1 件（たなか）
    thread(
        &threads,
        taro,
        coaches[0],
        opened_at,
        &[(true, start - hour * 24)],
    )
    .await;
    thread(
        &threads,
        taro,
        coaches[1],
        opened_at,
        &[(true, start - hour * 23)],
    )
    .await;
    thread(
        &threads,
        taro,
        coaches[2],
        opened_at,
        &[(true, start - hour * 50), (false, start - hour)],
    )
    .await;
    thread(&threads, taro, coaches[3], opened_at, &[]).await;
    thread(
        &threads,
        jiro,
        coaches[0],
        opened_at,
        &[(true, start - hour * 72)],
    )
    .await;
    let clock = ManualClock::new(start);
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    for (now, expected) in [(start, 1), (start + hour, 2)] {
        clock.set(now);
        for (student_id, expected) in [(&students[0], expected), (&students[1], 1)] {
            let (status, activity) = send(
                &router,
                "GET",
                &format!("/students/{student_id}/activity"),
                Some(&headquarters),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{activity}");
            assert_eq!(
                activity["unanswered_message_thread_count"], expected,
                "{student_id}: 24 時間ちょうどは含み、それに満たない・コーチが返した・発言のない・他の生徒のスレッドは数えない"
            );
            assert!(
                !activity.to_string().contains("本部には見せない本文"),
                "メッセージの本文は載せない: {activity}"
            );
            // 既定の条件の一覧と同じ数
            let (status, list) = send(
                &router,
                "GET",
                &format!("/message-threads/unanswered?student_id={student_id}"),
                Some(&headquarters),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{list}");
            assert_eq!(
                list["threads"].as_array().unwrap().len(),
                expected,
                "{student_id}: {list}"
            );
        }
    }
}

/// 生徒 `student` がコーチ `coach` に `submitted_at` に点 `rating` で付けた評価を保存先に直に書き（評価は完了したレッスンが
/// 要り、そこまでの HTTP の流れは `lessons.rs` で通している。`coaches.rs` と同じ）、評価の ID を返す。
async fn review(
    reviews: &PgLessonReviewRepository,
    student: AccountId,
    coach: AccountId,
    lesson: u128,
    rating: i64,
    submitted_at: SystemTime,
) -> String {
    let id = reviews.next_id();
    reviews
        .insert(LessonReview::restore(
            id,
            LessonCompletionId::from_u128(lesson),
            student,
            coach,
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("本部には見せない評価の本文").unwrap(),
            submitted_at,
        ))
        .await
        .unwrap();
    lesson_review_id_uuid::format(id)
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn counts_low_rated_reviews_by_the_student_as_the_low_rated_list_from_postgres(pool: PgPool) {
    let mut coaches = Vec::new();
    for (email, name) in [
        ("tanaka@example.com", "たなか"),
        ("ito@example.com", "いとう"),
    ] {
        let (coach_id, _) = register(&pool, email, name, Role::Coach, vec![Sport::Tennis]).await;
        coaches.push(account_id_uuid::parse(&coach_id).unwrap());
    }
    let (_, headquarters) =
        register(&pool, "hq@example.com", "本部", Role::Headquarters, vec![]).await;
    let mut students = Vec::new();
    for (email, name) in [
        ("taro@example.com", "たろう"),
        ("jiro@example.com", "じろう"),
        ("saburo@example.com", "さぶろう"),
    ] {
        let (student_id, _) =
            register(&pool, email, name, Role::Student, vec![Sport::Tennis]).await;
        students.push(student_id);
    }
    let taro = account_id_uuid::parse(&students[0]).unwrap();
    let jiro = account_id_uuid::parse(&students[1]).unwrap();
    let day = 24 * 3600;
    // たろうは日 0 に ★2（たなか）、日 10 に ★1（たなか）と ★2（いとう）と ★3（たなか）。じろうは日 10 に ★1（いとう）。
    // さぶろうは評価していない
    let reviews = PgLessonReviewRepository::new(pool.clone());
    review(&reviews, taro, coaches[0], 1, 2, at(0)).await;
    review(&reviews, taro, coaches[0], 2, 1, at(10 * day)).await;
    let ito_review = review(&reviews, taro, coaches[1], 3, 2, at(10 * day)).await;
    review(&reviews, taro, coaches[0], 4, 3, at(10 * day)).await;
    review(&reviews, jiro, coaches[1], 5, 1, at(10 * day)).await;
    let clock = ManualClock::new(at(20 * day));
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    /// 生徒 1 人の利用の様子の（低い評価の件数, 未対応の件数）と、低い評価の一覧をその生徒で絞った
    /// （件数, `handled=false` も付けた件数）を読む。
    async fn counts(router: &Router, headquarters: &str, student_id: &str) -> [(u64, u64); 2] {
        let (status, activity) = send(
            router,
            "GET",
            &format!("/students/{student_id}/activity"),
            Some(headquarters),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{activity}");
        assert!(
            !activity.to_string().contains("本部には見せない評価の本文"),
            "評価の本文は載せない: {activity}"
        );
        let mut listed = [0, 0];
        for (n, query) in ["", "&handled=false"].into_iter().enumerate() {
            let (status, list) = send(
                router,
                "GET",
                &format!("/lesson-reviews/low?student_id={student_id}{query}"),
                Some(headquarters),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{list}");
            listed[n] = list["reviews"].as_array().unwrap().len() as u64;
        }
        [
            (
                activity["low_rated_review_count"].as_u64().unwrap(),
                activity["unhandled_low_rated_review_count"]
                    .as_u64()
                    .unwrap(),
            ),
            (listed[0], listed[1]),
        ]
    }

    for (student_id, expected) in [
        (&students[0], (3, 3)),
        (&students[1], (1, 1)),
        (&students[2], (0, 0)),
    ] {
        assert_eq!(
            counts(&router, &headquarters, student_id).await,
            [expected, expected],
            "{student_id}: ★3・他の生徒の評価は数えず、評価されたコーチを問わない。その生徒で絞った一覧と同じ数"
        );
    }

    // 本部が対応済みの印を付けると、未対応の件数から外れる（低い評価の件数は変わらない）
    let (status, body) = send(
        &router,
        "PUT",
        &format!("/lesson-reviews/{ito_review}/handling"),
        Some(&headquarters),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        counts(&router, &headquarters, &students[0]).await,
        [(3, 2), (3, 2)]
    );

    // 照会した時刻が進んで、日 0 の ★2 が 30 日の外になれば外れる（30 日前ちょうどは含む）
    clock.set(at(30 * day));
    assert_eq!(
        counts(&router, &headquarters, &students[0]).await,
        [(3, 2), (3, 2)],
        "30 日前ちょうどは含む"
    );
    clock.set(at(30 * day + 1));
    assert_eq!(
        counts(&router, &headquarters, &students[0]).await,
        [(2, 1), (2, 1)]
    );
}
