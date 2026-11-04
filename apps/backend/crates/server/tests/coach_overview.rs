//! 本部がコーチ 1 人の様子（admin-web の `getCoachOverview`）を読むことを、保存先を Postgres にした組み立て
//! （`server::router_with_postgres`）で通す。ルートが組み立てに入っていること、受付の状態・評価の件数と平均・直近の低い評価の
//! 件数と未対応の件数（本部が `PUT /lesson-reviews/{review_id}/handling` で付けた印を読む）・今月と先月の実績・公開中の
//! レッスンメニューの数・直近 30 日に送られた動画の返却までの時間の中央値とそのうち返した件数・返却待ちの動画添削の件数と
//! そのうち 48 時間以上たった件数・直近 30 日にキャンセルされた予約の数を、それぞれの保存先
//! （scheduling・review・lesson・catalog・async-lesson）から読むこと、返信のないまま既定の時間の過ぎたスレッドの数を
//! messaging の保存先から読み、`listUnansweredMessageThreads` の既定の条件の一覧と同じ数になること、照会のあとも何も変わらないこと、ないコーチは 404・
//! 本部以外は 403 になることを確かめる。契約の形と数え方の細かい検査は interface の
//! `tests/coach_overviews.rs` と各操作のテスト。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test coach_overview -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    async_lesson::VideoRepository,
    clock::SystemClock,
    identity::{
        RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer, account_id_uuid,
    },
    in_memory::{FixedClock, ManualClock, RecordingAnalyticsSink},
    messaging::MessageThreadRepository,
    review::{LessonReviewRepository, lesson_review_id_uuid},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    async_lesson::{ReviewComment, Video},
    catalog::Sport,
    identity::{AccountId, Role},
    lesson::LessonCompletionId,
    messaging::{MessageBody, MessageContent, MessageThread},
    review::{LessonReview, LessonReviewComment, LessonReviewId, Rating},
};
use infra::{
    MockTokenService, PgAccountRepository, PgLessonReviewRepository, PgMessageThreadRepository,
    PgVideoRepository,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

/// 2026-10-26T00:00:00Z（日本時間の 10 月 26 日 9 時）
const NOW: Duration = Duration::from_secs(1_792_972_800);
const HOUR: Duration = Duration::from_secs(3600);

async fn send(
    router: &Router,
    method: &str,
    path: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json");
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

/// アカウントを登録して ID とトークンを返す。契約にコーチ・本部の登録の操作がないので、ユースケースを直に呼ぶ。
async fn register(pool: &PgPool, email: &str, role: Role) -> (AccountId, String) {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        SystemClock,
    )
    .execute(RegisterAccountCommand {
        email: email.to_string(),
        display_name: email.split('@').next().unwrap().to_string(),
        role,
        sports: if role == Role::Headquarters {
            vec![]
        } else {
            vec![Sport::Tennis]
        },
    })
    .await
    .unwrap();
    let token = MockTokenService
        .issue(TokenClaims {
            account_id: account.id(),
            role,
        })
        .into_string();
    (account.id(), token)
}

/// 生徒 `student` がコーチ `coach` に `submitted_at` に付けた評価を保存先に書き、評価の ID を返す（`lesson_id` には外部キーが
/// ない）。
async fn review(
    reviews: &PgLessonReviewRepository,
    student: AccountId,
    coach: AccountId,
    lesson: u128,
    rating: i64,
    submitted_at: SystemTime,
) -> LessonReviewId {
    let review_id = reviews.next_id();
    reviews
        .insert(LessonReview::restore(
            review_id,
            LessonCompletionId::from_u128(lesson),
            student,
            coach,
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("よい").unwrap(),
            submitted_at,
        ))
        .await
        .unwrap();
    review_id
}

/// 生徒 `student` が `submitted_at` にコーチ `coach` に送った動画を保存先に書き、`returned_at` が `Some` ならその時刻に
/// コーチが添削を返したことにする。
async fn video(
    videos: &PgVideoRepository,
    student: AccountId,
    coach: AccountId,
    submitted_at: SystemTime,
    returned_at: Option<SystemTime>,
) {
    let video_id = videos.next_video_id();
    videos
        .insert(Video::submit(
            video_id,
            student,
            coach,
            Sport::Tennis,
            submitted_at,
        ))
        .await
        .unwrap();
    if let Some(returned_at) = returned_at {
        let review_id = videos.next_review_id();
        videos
            .update(video_id, |video| {
                video.return_review(
                    review_id,
                    coach,
                    ReviewComment::new("よい").unwrap(),
                    true,
                    returned_at,
                )
            })
            .await
            .unwrap();
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_reads_the_overview_of_a_coach_from_postgres(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (ito, _) = register(&pool, "ito@example.com", Role::Coach).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;
    let (_, hq_token) = register(&pool, "hq@example.com", Role::Headquarters).await;

    let now = SystemTime::UNIX_EPOCH + NOW;
    let reviews = PgLessonReviewRepository::new(pool.clone());
    review(&reviews, taro, tanaka, 1, 4, now - HOUR).await;
    // 2026-09-30T00:00:00Z（日本時間の 9 月 30 日 9 時）は先月
    review(&reviews, taro, tanaka, 2, 5, now - HOUR * 24 * 26).await;
    review(&reviews, taro, ito, 3, 1, now - HOUR).await;
    // 直近 30 日の低い評価は★2 と★1 の 2 件（★2 には本部があとで対応済みの印を付ける）。31 日前の★1 は数えない
    let handled = review(&reviews, taro, tanaka, 4, 2, now - HOUR * 2).await;
    review(&reviews, taro, tanaka, 5, 1, now - HOUR * 3).await;
    review(&reviews, taro, tanaka, 6, 1, now - HOUR * 24 * 31).await;
    // 直近 30 日に送られた動画は、送付から 2・7・30 時間で返した 3 本と、10 時間前に送られてまだ返していない 1 本（中央値は
    // 2・7・10・30 時間の真ん中の 2 つの平均で 8.5 時間）。30 日と 1 時間前に送られた 1 本は、返却が期間内でも数えない
    let videos = PgVideoRepository::new(pool.clone());
    let day = HOUR * 24;
    for (submitted_at, lead) in [
        (now - HOUR * 3, HOUR * 2),
        (now - day * 3 - HOUR * 7, HOUR * 7),
        (now - day * 29, HOUR * 30),
        (now - day * 30 - HOUR, day),
    ] {
        video(
            &videos,
            taro,
            tanaka,
            submitted_at,
            Some(submitted_at + lead),
        )
        .await;
    }
    video(&videos, taro, tanaka, now - HOUR * 10, None).await;
    // 返却待ちは 10 時間前の 1 本と、31 日前に送られてまだ返していない 1 本（返却までの時間の期間の外なので中央値には入らない
    // が、返却待ちには数え、48 時間以上たっている）
    video(&videos, taro, tanaka, now - day * 31, None).await;
    video(&videos, taro, ito, now - HOUR * 5, Some(now - HOUR)).await;
    let router =
        server::router_with_postgres(pool, FixedClock(now), RecordingAnalyticsSink::default());

    // メニューは公開中 2（うち 1 つを非公開に直す）と、ほかに公開中 1
    let mut menu_ids = Vec::new();
    for name in ["フォームの添削", "サーブの添削", "ボレーの添削"] {
        let (status, menu) = send(
            &router,
            "POST",
            "/coaches/me/lesson-menus",
            &tanaka_token,
            Some(json!({ "sport": "tennis", "lesson_type": "async_lesson", "name": name })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{menu}");
        menu_ids.push(menu["lesson_menu_id"].as_str().unwrap().to_string());
    }
    let (status, body) = send(
        &router,
        "PUT",
        &format!("/coaches/me/lesson-menus/{}", menu_ids[2]),
        &tanaka_token,
        Some(json!({
            "sport": "tennis",
            "lesson_type": "async_lesson",
            "name": "ボレーの添削",
            "description": "",
            "is_published": false
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tanaka_id = account_id_uuid::format(tanaka);
    let (status, body) = send(
        &router,
        "POST",
        &format!("/coaches/{tanaka_id}/suspend"),
        &hq_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let path = format!("/coaches/{tanaka_id}/overview");
    let (status, body) = send(&router, "GET", &path, &hq_token, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            &body["low_rated_review_count"],
            &body["unhandled_low_rated_review_count"]
        ),
        (&json!(2), &json!(2)),
        "印を付ける前は 2 件とも未対応"
    );
    let (status, body) = send(
        &router,
        "PUT",
        &format!(
            "/lesson-reviews/{}/handling",
            lesson_review_id_uuid::format(handled)
        ),
        &hq_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = send(&router, "GET", &path, &hq_token, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let expected = json!({
        "coach_id": tanaka_id,
        "display_name": "tanaka",
        "acceptance_status": "suspended",
        "review_count": 5,
        "average_rating": 2.6,
        "low_rated_review_count": 2,
        "unhandled_low_rated_review_count": 1,
        "current_month": {
            "month": "2026-10",
            "live_lesson_count": 0,
            "async_lesson_count": 0,
            "review_count": 3,
            "average_rating": 2.33
        },
        "previous_month": {
            "month": "2026-09",
            "live_lesson_count": 0,
            "async_lesson_count": 0,
            "review_count": 2,
            "average_rating": 3.0
        },
        "published_lesson_menu_count": 2,
        "returned_video_review_count": 3,
        "median_video_review_lead_time_hours": 8.5,
        "pending_video_review_count": 2,
        "overdue_pending_video_review_count": 1,
        "recent_cancelled_booking_count": 0,
        "unanswered_message_thread_count": 0
    });
    assert_eq!(body, expected);
    // 照会だけで何も変えない（2 回目も同じ）
    let (_, again) = send(&router, "GET", &path, &hq_token, None).await;
    assert_eq!(again, expected);

    let (status, body) = send(
        &router,
        "GET",
        &format!("/coaches/{}/overview", account_id_uuid::format(ito)),
        &hq_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["acceptance_status"], "accepting");
    assert_eq!(body["review_count"], 1);
    assert_eq!(body["low_rated_review_count"], 1);
    assert_eq!(body["unhandled_low_rated_review_count"], 1);
    assert_eq!(body["published_lesson_menu_count"], 0);
    assert_eq!(body["returned_video_review_count"], 1);
    assert_eq!(body["median_video_review_lead_time_hours"], 4.0);
    assert_eq!(
        (
            &body["pending_video_review_count"],
            &body["overdue_pending_video_review_count"]
        ),
        (&json!(0), &json!(0)),
        "返却済みの動画は数えない"
    );

    let student_path = format!("/coaches/{}/overview", account_id_uuid::format(taro));
    let (status, _) = send(&router, "GET", &student_path, &hq_token, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    for token in [&tanaka_token, &taro_token] {
        let (status, _) = send(&router, "GET", &path, token, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn counts_bookings_of_the_coach_cancelled_in_the_last_30_days_from_postgres(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (_, ito_token) = register(&pool, "ito@example.com", Role::Coach).await;
    let (_, taro_token) = register(&pool, "taro@example.com", Role::Student).await;
    let (_, hq_token) = register(&pool, "hq@example.com", Role::Headquarters).await;
    let start = SystemTime::UNIX_EPOCH + NOW;
    let clock = ManualClock::new(start);
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    // たなかの枠を 4 つ、いとうの枠を 1 つ開け、生徒がすべて予約する（枠の開始は 2099 年）
    let mut bookings = Vec::new();
    for (i, token) in [
        &tanaka_token,
        &tanaka_token,
        &tanaka_token,
        &tanaka_token,
        &ito_token,
    ]
    .into_iter()
    .enumerate()
    {
        let (status, slot) = send(
            &router,
            "POST",
            "/slots",
            token,
            Some(json!({
                "sport": "tennis",
                "starts_at": format!("2099-01-0{}T10:00:00Z", i + 1),
                "ends_at": format!("2099-01-0{}T11:00:00Z", i + 1),
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{slot}");
        let (status, booking) = send(
            &router,
            "POST",
            "/bookings",
            &taro_token,
            Some(json!({ "slot_id": slot["slot_id"], "is_trial": false })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{booking}");
        bookings.push(booking["booking_id"].as_str().unwrap().to_string());
    }
    // たなかの予約は、照会の 30 日と 1 秒前・30 日前ちょうど・1 時間前にキャンセルし、1 件は残す。いとうの予約もキャンセルする
    let day = HOUR * 24;
    let query_at = start + Duration::from_secs(1) + day * 30;
    for (booking, cancelled_at) in [
        (&bookings[0], start),
        (&bookings[1], start + Duration::from_secs(1)),
        (&bookings[2], query_at - HOUR),
        (&bookings[4], query_at - HOUR),
    ] {
        clock.set(cancelled_at);
        let (status, body) = send(
            &router,
            "POST",
            &format!("/bookings/{booking}/cancel"),
            &taro_token,
            Some(json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    clock.set(query_at);
    let path = format!("/coaches/{}/overview", account_id_uuid::format(tanaka));
    let (status, body) = send(&router, "GET", &path, &hq_token, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["recent_cancelled_booking_count"], 2,
        "30 日前ちょうどは含み、それより前のキャンセル・有効な予約・他のコーチの予約は数えない"
    );

    // 期間は照会した時刻から数える（1 秒後には 30 日前ちょうどだったキャンセルが外れる）
    clock.set(query_at + Duration::from_secs(1));
    let (status, body) = send(&router, "GET", &path, &hq_token, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["recent_cancelled_booking_count"], 1);
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
async fn counts_unanswered_message_threads_of_the_coach_as_the_default_list_from_postgres(
    pool: PgPool,
) {
    let (tanaka, _) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (ito, _) = register(&pool, "ito@example.com", Role::Coach).await;
    let (_, hq_token) = register(&pool, "hq@example.com", Role::Headquarters).await;
    let mut students = Vec::new();
    for name in ["taro", "jiro", "saburo", "shiro"] {
        let (student, _) = register(&pool, &format!("{name}@example.com"), Role::Student).await;
        students.push(student);
    }
    let start = SystemTime::UNIX_EPOCH + NOW;
    let threads = PgMessageThreadRepository::new(pool.clone());
    let opened_at = start - HOUR * 24 * 10;
    // たなかのスレッドは、照会の 24 時間ちょうど前に生徒が送った 1 件・23 時間前に送った 1 件・コーチが返した 1 件・
    // 発言のない 1 件。いとうのスレッドは 72 時間前に生徒が送った 1 件
    thread(
        &threads,
        students[0],
        tanaka,
        opened_at,
        &[(true, start - HOUR * 24)],
    )
    .await;
    thread(
        &threads,
        students[1],
        tanaka,
        opened_at,
        &[(true, start - HOUR * 23)],
    )
    .await;
    thread(
        &threads,
        students[2],
        tanaka,
        opened_at,
        &[(true, start - HOUR * 50), (false, start - HOUR)],
    )
    .await;
    thread(&threads, students[3], tanaka, opened_at, &[]).await;
    thread(
        &threads,
        students[0],
        ito,
        opened_at,
        &[(true, start - HOUR * 72)],
    )
    .await;
    let clock = ManualClock::new(start);
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    for (at, expected) in [(start, 1), (start + HOUR, 2)] {
        clock.set(at);
        for (coach, expected) in [(tanaka, expected), (ito, 1)] {
            let coach_id = account_id_uuid::format(coach);
            let (status, overview) = send(
                &router,
                "GET",
                &format!("/coaches/{coach_id}/overview"),
                &hq_token,
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{overview}");
            assert_eq!(
                overview["unanswered_message_thread_count"], expected,
                "{coach_id}: 24 時間ちょうどは含み、それに満たない・コーチが返した・発言のない・他のコーチのスレッドは数えない"
            );
            // 既定の条件の一覧と同じ数
            let (status, list) = send(
                &router,
                "GET",
                &format!("/message-threads/unanswered?coach_id={coach_id}"),
                &hq_token,
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{list}");
            assert_eq!(
                list["threads"].as_array().unwrap().len(),
                expected,
                "{coach_id}: {list}"
            );
        }
    }
}
