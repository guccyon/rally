//! 生徒の次の操作（student-app の `GET /me/next-actions`。ADR 0010）を組み立て済みのルーターで通す。インメモリの組み立て
//! （`server::router()`）と、保存先を Postgres にした組み立て（`server::router_with_postgres`）の両方で、コーチが枠を
//! 公開し、登録した生徒が体験予約すると確定待ち（`confirm_booking`）が、確定すると次の同期レッスン（`join_live_lesson`）が
//! 返ること・ほかの生徒には返らないこと・生徒以外は 403、トークンがなければ 401 であることを確かめる。決め方と契約の形の
//! 細かい検査は domain の `scheduling::next_action` と interface の `tests/student_next_actions.rs`。
//!
//! Postgres の組み立ては既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test student_next_actions -- --include-ignored` で走らせる。

use application::{
    clock::SystemClock,
    identity::{RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer},
    in_memory::RecordingAnalyticsSink,
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

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn post_json(path: &str, token: Option<&str>, body: Value) -> Request<Body> {
    let mut request = Request::post(path).header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    request.body(Body::from(body.to_string())).unwrap()
}

fn next_actions(token: Option<&str>) -> Request<Body> {
    let mut request = Request::get("/me/next-actions");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    request.body(Body::empty()).unwrap()
}

async fn register(router: &Router, email: &str, display_name: &str) -> String {
    let (status, body) = send(
        router,
        post_json(
            "/accounts",
            None,
            json!({ "email": email, "display_name": display_name, "sport": "tennis" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["access_token"].as_str().unwrap().to_string()
}

/// 役割 `role` のトークン（アカウントは置かない。インメモリの組み立て用）。
fn token(id: u128, role: Role) -> String {
    MockTokenService
        .issue(TokenClaims {
            account_id: AccountId::from_u128(id),
            role,
        })
        .into_string()
}

/// 役割 `role` のアカウントを Postgres に登録してトークンを返す（コーチの提供スポーツはテニス）。契約にコーチ・本部の登録の操作がないので、ユースケースを
/// 直に呼ぶ。
async fn register_in_postgres(pool: &PgPool, email: &str, role: Role) -> String {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        SystemClock,
    )
    .execute(RegisterAccountCommand {
        email: email.to_string(),
        display_name: "たなか".to_string(),
        role,
        sports: if role == Role::Coach {
            vec![Sport::Tennis]
        } else {
            vec![]
        },
    })
    .await
    .unwrap();
    MockTokenService
        .issue(TokenClaims {
            account_id: account.id(),
            role,
        })
        .into_string()
}

/// 予約から次の操作が返るまでを `router` で通す。`coach`・`headquarters` はコーチ・本部のトークン。
async fn booking_flow_returns_next_actions(router: Router, coach: String, headquarters: String) {
    let student = register(&router, "taro@example.com", "たろう").await;
    let other = register(&router, "hanako@example.com", "はなこ").await;

    // 記録がなければ空の配列
    let (status, body) = send(&router, next_actions(Some(&student))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "next_actions": [] }));

    let (status, slot) = send(
        &router,
        post_json(
            "/slots",
            Some(&coach),
            json!({
                "sport": "tennis",
                "starts_at": "2099-01-01T10:00:00Z",
                "ends_at": "2099-01-01T11:00:00Z",
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{slot}");
    let (status, booking) = send(
        &router,
        post_json(
            "/bookings",
            Some(&student),
            json!({ "slot_id": slot["slot_id"], "is_trial": true }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");

    let (status, body) = send(&router, next_actions(Some(&student))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let actions = body["next_actions"].as_array().unwrap();
    assert_eq!(actions.len(), 1, "{body}");
    assert_eq!(actions[0]["kind"], "confirm_booking");
    assert_eq!(actions[0]["booking_id"], booking["booking_id"]);
    assert_eq!(actions[0]["is_trial"], true);
    assert_eq!(actions[0]["starts_at"], "2099-01-01T10:00:00Z");
    assert_eq!(actions[0]["sport"], "tennis");

    // ほかの生徒には返らない
    let (status, body) = send(&router, next_actions(Some(&other))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "next_actions": [] }));

    let path = format!(
        "/bookings/{}/confirm",
        booking["booking_id"].as_str().unwrap()
    );
    let (status, body) = send(&router, post_json(&path, Some(&student), json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // 確定すると、次の同期レッスン（まだ開始の 5 分前より前）
    let (status, body) = send(&router, next_actions(Some(&student))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let actions = body["next_actions"].as_array().unwrap();
    assert_eq!(actions.len(), 1, "{body}");
    assert_eq!(actions[0]["kind"], "join_live_lesson");
    assert_eq!(actions[0]["booking_id"], booking["booking_id"]);

    for token in [&coach, &headquarters] {
        let (status, body) = send(&router, next_actions(Some(token))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
    let (status, body) = send(&router, next_actions(None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
}

#[tokio::test]
async fn next_actions_follow_the_booking_in_memory() {
    booking_flow_returns_next_actions(
        server::router(),
        token(42, Role::Coach),
        token(43, Role::Headquarters),
    )
    .await;
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn next_actions_follow_the_booking_in_postgres(pool: PgPool) {
    let coach = register_in_postgres(&pool, "coach@example.com", Role::Coach).await;
    let headquarters = register_in_postgres(&pool, "hq@example.com", Role::Headquarters).await;
    booking_flow_returns_next_actions(
        server::router_with_postgres(pool, SystemClock, RecordingAnalyticsSink::default()),
        coach,
        headquarters,
    )
    .await;
}
