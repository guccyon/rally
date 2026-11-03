//! 本部が見る生徒の一覧（admin-web の `GET /students`）を、保存先を Postgres にした組み立て
//! （`server::router_with_postgres`）で通す。生徒が student-app の操作で登録し・月額契約を始め・予約したことが、
//! 一覧の契約種別と契約プラン・最後に予約した時刻・直近 30 日の予約の数に出ること、スポーツと最後に予約した日（日本時間）で絞れること、
//! 予約したことのあるコーチで絞れること（キャンセルした予約も数える）、Postgres に保存したスレッドのうち返信のないまま
//! 既定の時間の過ぎたものがある生徒だけに絞れること（`getStudentActivity` の `unanswered_message_thread_count` と揃う）、
//! コーチ・生徒は 403 であることを確かめる。契約の形の
//! 細かい検査は interface の `tests/students.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test students -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    identity::{
        RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer, account_id_uuid,
    },
    in_memory::{ManualClock, RecordingAnalyticsSink},
    messaging::MessageThreadRepository,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    identity::{AccountId, Role},
    messaging::{MessageBody, MessageContent, MessageThread},
};
use infra::{MockTokenService, PgAccountRepository, PgMessageThreadRepository};
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

/// 役割 `role` のアカウントをユースケースを直に呼んで登録し（契約にコーチ・本部の登録の操作がない）、トークンを返す。
async fn register(pool: &PgPool, email: &str, role: Role, sports: Vec<Sport>) -> String {
    register_with_id(pool, email, role, sports).await.1
}

/// [`register`] と同じく登録し、アカウントの ID（UUID の文字列）とトークンを返す。
async fn register_with_id(
    pool: &PgPool,
    email: &str,
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
        display_name: "テスト".to_string(),
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

/// student-app の `POST /accounts` で生徒を登録し、生徒の ID とトークンを返す。
async fn register_student(router: &Router, email: &str, sport: &str) -> (String, String) {
    let (status, body) = send(
        router,
        "POST",
        "/accounts",
        None,
        Some(json!({ "email": email, "display_name": "せいと", "sport": sport })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    (
        body["student"]["student_id"].as_str().unwrap().to_string(),
        body["access_token"].as_str().unwrap().to_string(),
    )
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_sees_contract_and_last_booking_of_students(pool: PgPool) {
    let coach = register(&pool, "coach@example.com", Role::Coach, vec![Sport::Tennis]).await;
    let headquarters = register(&pool, "hq@example.com", Role::Headquarters, vec![]).await;
    let clock = ManualClock::new(at(60));
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    let (subscriber_id, subscriber) = register_student(&router, "taro@example.com", "tennis").await;
    clock.set(at(120));
    let (newcomer_id, _) = register_student(&router, "hanako@example.com", "swimming").await;

    // 先に登録した生徒は月額（plus）を始め、予約する
    clock.set(at(180));
    let (status, body) = send(
        &router,
        "POST",
        "/subscriptions",
        Some(&subscriber),
        Some(json!({ "sport": "tennis", "plan": "plus", "payment_method_token": "tok_mock" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, slot) = send(
        &router,
        "POST",
        "/slots",
        Some(&coach),
        Some(json!({
            "sport": "tennis",
            "starts_at": "2099-01-01T10:00:00Z",
            "ends_at": "2099-01-01T11:00:00Z",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{slot}");
    clock.set(at(240));
    let (status, body) = send(
        &router,
        "POST",
        "/bookings",
        Some(&subscriber),
        Some(json!({ "slot_id": slot["slot_id"], "is_trial": false })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, body) = send(&router, "GET", "/students", Some(&headquarters), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "students": [
                {
                    "student_id": newcomer_id,
                    "display_name": "せいと",
                    "sport": "swimming",
                    "contract_type": null,
                    "plan": null,
                    "last_booked_at": null,
                    "recent_booking_count": 0,
                    "registered_at": "2026-10-05T00:02:00Z",
                },
                {
                    "student_id": subscriber_id,
                    "display_name": "せいと",
                    "sport": "tennis",
                    "contract_type": "subscription",
                    "plan": "plus",
                    "last_booked_at": "2026-10-05T00:04:00Z",
                    "recent_booking_count": 1,
                    "registered_at": "2026-10-05T00:01:00Z",
                },
            ],
        })
    );

    // 直近の予約の数は時計の今の時刻で数える。予約を作ってちょうど 30 日後までは数え、それより後は数えない
    const THIRTY_DAYS: u64 = 30 * 24 * 3600;
    for (now, expected) in [(at(240 + THIRTY_DAYS), 1), (at(240 + THIRTY_DAYS + 1), 0)] {
        clock.set(now);
        let (status, body) = send(&router, "GET", "/students", Some(&headquarters), None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["students"][1]["recent_booking_count"], expected);
        assert_eq!(
            body["students"][1]["last_booked_at"],
            "2026-10-05T00:04:00Z"
        );
    }
    clock.set(at(240));

    // スポーツと、その日（日本時間）以降に予約していないかで絞る。予約は日本時間の 2026-10-05 09:04
    for (path, expected) in [
        ("/students?sport=tennis", vec![&subscriber_id]),
        ("/students?sport=swimming", vec![&newcomer_id]),
        ("/students?sport=golf", vec![]),
        ("/students?not_booked_since=2026-10-05", vec![&newcomer_id]),
        (
            "/students?not_booked_since=2026-10-06",
            vec![&newcomer_id, &subscriber_id],
        ),
        ("/students?sport=tennis&not_booked_since=2026-10-05", vec![]),
        (
            "/students?sport=tennis&not_booked_since=2026-10-06",
            vec![&subscriber_id],
        ),
    ] {
        let (status, body) = send(&router, "GET", path, Some(&headquarters), None).await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
        let listed: Vec<&str> = body["students"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["student_id"].as_str().unwrap())
            .collect();
        assert_eq!(listed, expected, "{path}");
    }
    let (status, body) = send(
        &router,
        "GET",
        "/students?sport=tennis&not_booked_since=2026-10-06",
        Some(&headquarters),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["students"][0]["contract_type"], "subscription");
    assert_eq!(
        body["students"][0]["last_booked_at"],
        "2026-10-05T00:04:00Z"
    );

    for token in [&coach, &subscriber] {
        let (status, body) = send(&router, "GET", "/students", Some(token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        let (status, body) = send(
            &router,
            "GET",
            "/students?sport=tennis&not_booked_since=2026-10-06",
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
}

/// 生徒 `student` が月額（standard）を始め、コーチ `coach` が 2099-01-01 の `hour` 時（UTC）から 1 時間の枠を公開し、
/// 生徒がその枠を予約する。予約の ID を返す。
async fn book_new_slot(router: &Router, student: &str, coach: &str, hour: u32) -> String {
    let (status, body) = send(
        router,
        "POST",
        "/subscriptions",
        Some(student),
        Some(json!({ "sport": "tennis", "plan": "standard", "payment_method_token": "tok_mock" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, slot) = send(
        router,
        "POST",
        "/slots",
        Some(coach),
        Some(json!({
            "sport": "tennis",
            "starts_at": format!("2099-01-01T{hour:02}:00:00Z"),
            "ends_at": format!("2099-01-01T{:02}:00:00Z", hour + 1),
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{slot}");
    let (status, body) = send(
        router,
        "POST",
        "/bookings",
        Some(student),
        Some(json!({ "slot_id": slot["slot_id"], "is_trial": false })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["booking_id"].as_str().unwrap().to_string()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_lists_students_who_booked_the_coach_including_cancelled_bookings(
    pool: PgPool,
) {
    let (coach_id, coach) =
        register_with_id(&pool, "coach@example.com", Role::Coach, vec![Sport::Tennis]).await;
    let other_coach = register(&pool, "other@example.com", Role::Coach, vec![Sport::Tennis]).await;
    let headquarters = register(&pool, "hq@example.com", Role::Headquarters, vec![]).await;
    let clock = ManualClock::new(at(60));
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    let (booked_id, booked) = register_student(&router, "taro@example.com", "tennis").await;
    clock.set(at(120));
    let (cancelled_id, cancelled) = register_student(&router, "jiro@example.com", "tennis").await;
    clock.set(at(180));
    let (other_id, other) = register_student(&router, "saburo@example.com", "tennis").await;
    clock.set(at(240));
    register_student(&router, "hanako@example.com", "tennis").await;

    // 3 人は月額を始め、コーチの枠・ほかのコーチの枠を予約する。1 人はあとでキャンセルする
    book_new_slot(&router, &booked, &coach, 10).await;
    let booking_id = book_new_slot(&router, &cancelled, &coach, 12).await;
    book_new_slot(&router, &other, &other_coach, 14).await;
    let (status, body) = send(
        &router,
        "POST",
        &format!("/bookings/{booking_id}/cancel"),
        Some(&cancelled),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let listed = |body: &Value| -> Vec<String> {
        body["students"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["student_id"].as_str().unwrap().to_string())
            .collect()
    };
    // 登録の新しい順はそのまま。キャンセルした生徒も載り、ほかのコーチだけを予約した生徒・予約のない生徒は載らない
    let path = format!("/students?coach_id={coach_id}");
    let (status, body) = send(&router, "GET", &path, Some(&headquarters), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(listed(&body), [cancelled_id.clone(), booked_id.clone()]);
    assert_eq!(body["students"][0]["contract_type"], "subscription");

    let path = format!("/students?coach_id={coach_id}&not_booked_since=2026-10-06&sport=tennis");
    let (status, body) = send(&router, "GET", &path, Some(&headquarters), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(listed(&body), [cancelled_id, booked_id]);
    let path = format!("/students?coach_id={coach_id}&sport=swimming");
    let (status, body) = send(&router, "GET", &path, Some(&headquarters), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(listed(&body).is_empty());

    // ない ID・コーチでない ID（生徒）は空の一覧。UUID でなければ 422
    for id in ["00000000-0000-0000-0000-000000000009", other_id.as_str()] {
        let path = format!("/students?coach_id={id}");
        let (status, body) = send(&router, "GET", &path, Some(&headquarters), None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(listed(&body).is_empty(), "{id}: {body}");
    }
    let (status, body) = send(
        &router,
        "GET",
        "/students?coach_id=coach",
        Some(&headquarters),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");
}

/// 生徒 `student` とコーチ `coach` のスレッドを `opened_at` に開いて保存先に書き、`exchange` の順に（生徒が送ったか, 送信時刻）で
/// 本文を送る（スレッドは予約の確定で開くが、そこまでの流れは `student_activity.rs` と同じく省いて直に書く）。
async fn thread(
    threads: &PgMessageThreadRepository,
    student: &str,
    coach: &str,
    opened_at: SystemTime,
    exchange: &[(bool, SystemTime)],
) {
    let student: AccountId = account_id_uuid::parse(student).unwrap();
    let coach: AccountId = account_id_uuid::parse(coach).unwrap();
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
async fn headquarters_lists_only_students_with_unanswered_message_threads_from_postgres(
    pool: PgPool,
) {
    let (coach_id, coach) =
        register_with_id(&pool, "coach@example.com", Role::Coach, vec![Sport::Tennis]).await;
    let (other_coach_id, _) =
        register_with_id(&pool, "other@example.com", Role::Coach, vec![Sport::Tennis]).await;
    let headquarters = register(&pool, "hq@example.com", Role::Headquarters, vec![]).await;
    let clock = ManualClock::new(at(60));
    let router = server::router_with_postgres(
        pool.clone(),
        clock.clone(),
        RecordingAnalyticsSink::default(),
    );

    let (taro, taro_token) = register_student(&router, "taro@example.com", "tennis").await;
    clock.set(at(120));
    let (jiro, _) = register_student(&router, "jiro@example.com", "tennis").await;
    clock.set(at(180));
    let (saburo, _) = register_student(&router, "saburo@example.com", "tennis").await;
    clock.set(at(240));
    let (hanako, _) = register_student(&router, "hanako@example.com", "swimming").await;
    clock.set(at(300));
    let (shiro, _) = register_student(&router, "shiro@example.com", "tennis").await;
    // たろうだけがコーチの枠を予約した
    book_new_slot(&router, &taro_token, &coach, 10).await;

    let start = at(30 * 24 * 3600);
    let hour = Duration::from_secs(3600);
    let opened_at = at(0);
    let threads = PgMessageThreadRepository::new(pool);
    // たろうは照会の 24 時間ちょうど前に送った（ほかのコーチとのスレッドはコーチが返した）。じろうは 23 時間前に送った。
    // さぶろうはコーチが返した。はなこ（水泳）は 72 時間前に送った。しろうはスレッドがない
    thread(
        &threads,
        &taro,
        &coach_id,
        opened_at,
        &[(true, start - hour * 24)],
    )
    .await;
    thread(
        &threads,
        &taro,
        &other_coach_id,
        opened_at,
        &[(true, start - hour * 50), (false, start - hour)],
    )
    .await;
    thread(
        &threads,
        &jiro,
        &coach_id,
        opened_at,
        &[(true, start - hour * 23)],
    )
    .await;
    thread(
        &threads,
        &saburo,
        &coach_id,
        opened_at,
        &[(true, start - hour * 50), (false, start - hour)],
    )
    .await;
    thread(
        &threads,
        &hanako,
        &coach_id,
        opened_at,
        &[(true, start - hour * 72)],
    )
    .await;

    let listed = |body: &Value| -> Vec<String> {
        body["students"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["student_id"].as_str().unwrap().to_string())
            .collect()
    };
    let get = |path: String| {
        let router = router.clone();
        let headquarters = headquarters.clone();
        async move {
            let (status, body) = send(&router, "GET", &path, Some(&headquarters), None).await;
            assert_eq!(status, StatusCode::OK, "{path}: {body}");
            body
        }
    };

    // 登録の新しい順はそのまま。照会した時刻から数え、1 時間後にはじろうも載る
    for (now, expected) in [
        (start, vec![hanako.clone(), taro.clone()]),
        (
            start + hour,
            vec![hanako.clone(), jiro.clone(), taro.clone()],
        ),
    ] {
        clock.set(now);
        let body = get("/students?has_unanswered=true".to_string()).await;
        assert_eq!(listed(&body), expected, "{body}");
        assert!(
            !body.to_string().contains("本部には見せない本文"),
            "メッセージの本文は載せない: {body}"
        );
        // 生徒 1 人の利用の様子の数が 1 以上の生徒と同じ
        for student in [&taro, &jiro, &saburo, &hanako, &shiro] {
            let activity = get(format!("/students/{student}/activity")).await;
            assert_eq!(
                activity["unanswered_message_thread_count"]
                    .as_u64()
                    .unwrap()
                    > 0,
                expected.contains(student),
                "{student}: {activity}"
            );
        }
    }
    clock.set(start);

    let body = get("/students?has_unanswered=false".to_string()).await;
    assert_eq!(
        listed(&body),
        [shiro.clone(), hanako.clone(), saburo, jiro, taro.clone()],
        "false は絞らない"
    );
    let body = get("/students?has_unanswered=true&sport=tennis".to_string()).await;
    assert_eq!(listed(&body), std::slice::from_ref(&taro));
    let body = get(format!("/students?has_unanswered=true&coach_id={coach_id}")).await;
    assert_eq!(listed(&body), std::slice::from_ref(&taro));
    assert_eq!(body["students"][0]["contract_type"], "subscription");
    let body = get(format!(
        "/students?has_unanswered=true&coach_id={coach_id}&sport=swimming"
    ))
    .await;
    assert!(listed(&body).is_empty());
    let body = get("/students?has_unanswered=true&not_booked_since=2026-10-05".to_string()).await;
    assert_eq!(
        listed(&body),
        [hanako],
        "日本時間の 2026-10-05 に予約したたろうは外れる"
    );

    let (status, body) = send(
        &router,
        "GET",
        "/students?has_unanswered=yes",
        Some(&headquarters),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");
}
