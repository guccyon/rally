//! 変更シナリオ cs-003 で足した月額契約のプラン（standard 暦月 2 回・plus 暦月 4 回）を、保存先を Postgres にした
//! 組み立て（`server::router_with_postgres`）で一周させる。生徒が `startSubscription` でプランを選び（省けば standard）、
//! 予約の確定でビデオ通話の暦月の枠を使い、プランの上限を超える確定は 409 `live_lesson_quota_exceeded`（不変条件 1）。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test subscription_plan -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    identity::{RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer},
    in_memory::{ManualClock, RecordingAnalyticsSink},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{Role, Sport};
use infra::{MockTokenService, PgAccountRepository};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

/// 2026-10-05T00:00:00+09:00（2026-10-04T15:00:00Z）
fn oct5_jst() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_791_126_000)
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

struct Scenario {
    router: Router,
    coach: String,
    student: String,
}

/// テニスのコーチと生徒を登録し、生徒が `request` で月額契約を開始した状態から始める。開始の応答を返す。
/// 契約にコーチの登録の操作がないので、コーチはユースケースを直に呼んで登録する。
async fn subscribed(pool: PgPool, request: Value) -> (Scenario, Value) {
    let coach = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        ManualClock::new(oct5_jst()),
    )
    .execute(RegisterAccountCommand {
        email: "coach@example.com".to_string(),
        display_name: "こーち".to_string(),
        role: Role::Coach,
        sports: vec![Sport::Tennis],
    })
    .await
    .unwrap();
    let coach = MockTokenService
        .issue(TokenClaims {
            account_id: coach.id(),
            role: Role::Coach,
        })
        .into_string();
    let router = server::router_with_postgres(
        pool,
        ManualClock::new(oct5_jst()),
        RecordingAnalyticsSink::default(),
    );
    let (status, account) = send(
        &router,
        "POST",
        "/accounts",
        None,
        Some(json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "tennis" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{account}");
    let student = account["access_token"].as_str().unwrap().to_string();
    let (status, subscription) = send(
        &router,
        "POST",
        "/subscriptions",
        Some(&student),
        Some(request),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{subscription}");
    (
        Scenario {
            router,
            coach,
            student,
        },
        subscription,
    )
}

impl Scenario {
    /// コーチが 12 月 `day` 日の 1 時間の枠を公開し、生徒がそれを予約して確定する。確定の応答を返す。
    async fn book_and_confirm(&self, day: u8) -> (StatusCode, Value) {
        let (status, slot) = send(
            &self.router,
            "POST",
            "/slots",
            Some(&self.coach),
            Some(json!({
                "sport": "tennis",
                "starts_at": format!("2026-12-{day:02}T01:00:00Z"),
                "ends_at": format!("2026-12-{day:02}T02:00:00Z"),
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{slot}");
        let (status, booking) = send(
            &self.router,
            "POST",
            "/bookings",
            Some(&self.student),
            Some(json!({ "slot_id": slot["slot_id"], "is_trial": false })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{booking}");
        let booking_id = booking["booking_id"].as_str().unwrap();
        send(
            &self.router,
            "POST",
            &format!("/bookings/{booking_id}/confirm"),
            Some(&self.student),
            Some(json!({})),
        )
        .await
    }

    async fn entitlement(&self) -> Value {
        let (status, entitlement) = send(
            &self.router,
            "GET",
            "/entitlements/me",
            Some(&self.student),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{entitlement}");
        entitlement
    }

    /// 上限まで確定でき、その次の確定は 409 `live_lesson_quota_exceeded` になることを確かめる。
    async fn assert_confirms_up_to(&self, limit: u8) {
        for day in 1..=limit {
            let (status, body) = self.book_and_confirm(day).await;
            assert_eq!(status, StatusCode::OK, "{day} 回目: {body}");
        }
        assert_eq!(
            self.entitlement().await["live_lesson_quota"],
            json!({ "month": "2026-10", "limit": limit, "used": limit, "remaining": 0 })
        );
        let (status, body) = self.book_and_confirm(limit + 1).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(
            body["code"],
            "live_lesson_quota_exceeded",
            "{} 回目は予約できない",
            limit + 1
        );
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn plus_confirms_four_live_lessons_a_month_and_rejects_the_fifth(pool: PgPool) {
    let (s, started) = subscribed(
        pool,
        json!({ "sport": "tennis", "plan": "plus", "payment_method_token": "tok_mock" }),
    )
    .await;
    assert_eq!(started["subscription"]["plan"], "plus");
    assert_eq!(started["entitlement"]["plan"], "plus");
    assert_eq!(
        started["entitlement"]["live_lesson_quota"],
        json!({ "month": "2026-10", "limit": 4, "used": 0, "remaining": 4 })
    );

    s.assert_confirms_up_to(4).await;
    assert_eq!(s.entitlement().await["plan"], "plus");
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn without_a_plan_the_subscription_is_standard_and_rejects_the_third(pool: PgPool) {
    let (s, started) = subscribed(
        pool,
        json!({ "sport": "tennis", "payment_method_token": "tok_mock" }),
    )
    .await;
    assert_eq!(started["subscription"]["plan"], "standard");
    assert_eq!(started["entitlement"]["plan"], "standard");

    s.assert_confirms_up_to(2).await;
}
