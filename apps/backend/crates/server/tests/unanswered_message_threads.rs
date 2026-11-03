//! 本部が返信のないまま時間の過ぎたスレッドの一覧を見る（admin-web の `listUnansweredMessageThreads`）ことを、保存先を
//! Postgres にした組み立て（`server::router_with_postgres`）で通す。ルートが組み立てに入っていること、API で送った
//! Postgres のメッセージから、最後の発言が生徒で既定の 24 時間（`older_than_hours` で変えられる）以上たったスレッドを
//! 本文なしで返し、コーチと生徒の表示名をアカウントの保存先から添えること、`coach_id` で担当コーチを絞れること（生徒の ID・
//! ない ID は空の一覧）、`student_id` で生徒を絞れること（コーチの ID・ない ID は空の一覧。`coach_id` と組み合わせられる）、
//! コーチが返せば消えること、本部以外は 403 になることを確かめる。契約の形・時間の範囲の細かい検査は interface の `tests/unanswered_message_threads.rs`、すべての
//! スレッドの読み取りは infra の `tests/pg_message_thread_repository.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test unanswered_message_threads -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    clock::SystemClock,
    identity::{
        RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer, account_id_uuid,
    },
    in_memory::{ManualClock, RecordingAnalyticsSink},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    identity::{AccountId, Role},
};
use infra::{MockTokenService, PgAccountRepository};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

const HOUR: Duration = Duration::from_secs(3600);
const PATH: &str = "/message-threads/unanswered";

/// 2026-10-30T00:00:00Z（シナリオの始まり）
fn oct30() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200) - 48 * HOUR
}

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn get(path: &str, token: &str) -> Request<Body> {
    Request::get(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

fn post_json(path: &str, token: &str, body: Value) -> Request<Body> {
    Request::post(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// `path` に `body` を POST して、`expected` の状態で返った本文。
async fn post(
    router: &Router,
    path: &str,
    token: &str,
    body: Value,
    expected: StatusCode,
) -> Value {
    let (status, body) = send(router, post_json(path, token, body)).await;
    assert_eq!(status, expected, "POST {path}: {body}");
    body
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

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_reads_unanswered_threads_from_postgres(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;
    let (_, hq_token) = register(&pool, "hq@example.com", Role::Headquarters).await;
    let clock = ManualClock::new(oct30());
    let pool_for_test = pool.clone();
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    // 予約の確定でスレッドが開き、月額契約で生徒が送れるようになる
    let slot = post(
        &router,
        "/slots",
        &tanaka_token,
        json!({ "sport": "tennis", "starts_at": "2026-11-05T10:00:00Z", "ends_at": "2026-11-05T11:00:00Z" }),
        StatusCode::CREATED,
    )
    .await;
    let booking = post(
        &router,
        "/bookings",
        &taro_token,
        json!({ "slot_id": slot["slot_id"], "is_trial": true }),
        StatusCode::CREATED,
    )
    .await;
    post(
        &router,
        &format!(
            "/bookings/{}/confirm",
            booking["booking_id"].as_str().unwrap()
        ),
        &taro_token,
        json!({}),
        StatusCode::OK,
    )
    .await;
    post(
        &router,
        "/subscriptions",
        &taro_token,
        json!({ "sport": "tennis", "payment_method_token": "tok_mock" }),
        StatusCode::CREATED,
    )
    .await;
    let (status, threads) = send(&router, get("/message-threads", &taro_token)).await;
    assert_eq!(status, StatusCode::OK, "{threads}");
    let thread_id = threads["threads"][0]["thread_id"].clone();
    let messages_path = format!("/message-threads/{}/messages", thread_id.as_str().unwrap());

    // 発言がまだないスレッドは出ない
    let (status, body) = send(&router, get(PATH, &hq_token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "older_than_hours": 24, "threads": [] }));

    // 1 時間後に生徒が送る
    clock.set(oct30() + HOUR);
    post(
        &router,
        &messages_path,
        &taro_token,
        json!({ "body": "本部には見せない質問" }),
        StatusCode::CREATED,
    )
    .await;

    // 23 時間 59 分では、まだ出ない
    clock.set(oct30() + 25 * HOUR - Duration::from_secs(60));
    let (status, body) = send(&router, get(PATH, &hq_token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "older_than_hours": 24, "threads": [] }));

    // 24 時間 30 分。本文は返さない
    clock.set(oct30() + 25 * HOUR + Duration::from_secs(30 * 60));
    let (status, body) = send(&router, get(PATH, &hq_token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "older_than_hours": 24,
            "threads": [{
                "thread_id": thread_id,
                "coach_id": account_id_uuid::format(tanaka),
                "coach_display_name": "tanaka",
                "student_id": account_id_uuid::format(taro),
                "student_display_name": "taro",
                "waiting_since": "2026-10-30T01:00:00Z",
                "elapsed_sec": 24 * 3600 + 30 * 60,
            }],
        })
    );

    // 生徒が催促しても、待ち始めと経過はリセットされない
    post(
        &router,
        &messages_path,
        &taro_token,
        json!({ "body": "まだでしょうか" }),
        StatusCode::CREATED,
    )
    .await;
    let (status, body) = send(&router, get(PATH, &hq_token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["threads"][0]["waiting_since"], "2026-10-30T01:00:00Z");
    assert_eq!(body["threads"][0]["elapsed_sec"], 24 * 3600 + 30 * 60);

    // 時間を変えられる
    let (status, body) = send(
        &router,
        get(&format!("{PATH}?older_than_hours=25"), &hq_token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "older_than_hours": 25, "threads": [] }));

    // 担当コーチで絞れる（時間と組み合わせられる）。生徒の ID・ない ID・ほかのコーチは空の一覧
    let (status, body) = send(
        &router,
        get(
            &format!("{PATH}?coach_id={}", account_id_uuid::format(tanaka)),
            &hq_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["older_than_hours"], 24);
    assert_eq!(body["threads"].as_array().unwrap().len(), 1, "{body}");
    assert_eq!(body["threads"][0]["thread_id"], thread_id);
    let (other_coach, _) = register(&pool_for_test, "suzuki@example.com", Role::Coach).await;
    for (query, coach) in [
        ("older_than_hours=24", taro),
        ("older_than_hours=24", other_coach),
        ("older_than_hours=24", AccountId::from_u128(12345)),
        ("older_than_hours=25", tanaka),
    ] {
        let (status, body) = send(
            &router,
            get(
                &format!("{PATH}?{query}&coach_id={}", account_id_uuid::format(coach)),
                &hq_token,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["threads"], json!([]), "{query} {coach:?}");
    }

    // 生徒で絞れる（時間・担当コーチと組み合わせられる）。コーチの ID・ない ID・ほかの生徒は空の一覧
    for query in [
        format!("student_id={}", account_id_uuid::format(taro)),
        format!(
            "student_id={}&coach_id={}&older_than_hours=24",
            account_id_uuid::format(taro),
            account_id_uuid::format(tanaka)
        ),
    ] {
        let (status, body) = send(&router, get(&format!("{PATH}?{query}"), &hq_token)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["older_than_hours"], 24);
        assert_eq!(
            body["threads"].as_array().unwrap().len(),
            1,
            "{query}: {body}"
        );
        assert_eq!(body["threads"][0]["thread_id"], thread_id);
    }
    let (other_student, _) = register(&pool_for_test, "hanako@example.com", Role::Student).await;
    for query in [
        format!("student_id={}", account_id_uuid::format(tanaka)),
        format!("student_id={}", account_id_uuid::format(other_student)),
        format!(
            "student_id={}",
            account_id_uuid::format(AccountId::from_u128(12345))
        ),
        format!(
            "older_than_hours=25&student_id={}",
            account_id_uuid::format(taro)
        ),
        format!(
            "student_id={}&coach_id={}",
            account_id_uuid::format(taro),
            account_id_uuid::format(other_coach)
        ),
    ] {
        let (status, body) = send(&router, get(&format!("{PATH}?{query}"), &hq_token)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["threads"], json!([]), "{query}");
    }

    // 本部だけ
    for token in [&tanaka_token, &taro_token] {
        let (status, body) = send(&router, get(PATH, token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }

    // コーチが返すと出なくなる
    post(
        &router,
        &messages_path,
        &tanaka_token,
        json!({ "body": "お返事が遅れました" }),
        StatusCode::CREATED,
    )
    .await;
    let (status, body) = send(&router, get(PATH, &hq_token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "older_than_hours": 24, "threads": [] }));
}
