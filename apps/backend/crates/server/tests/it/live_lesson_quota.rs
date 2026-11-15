//! 月額契約の生徒のビデオ通話の暦月の枠（不変条件 1。上限は契約プランで決まる）を、組み立て済みのルーター
//! （`server::router_at`）で一周させる。境界間イベントは infra のインメモリのイベントバスで entitlement の購読に
//! つながっている。
//!
//! - プランを選ばない契約（standard）は、同じ暦月に 3 回目の予約の確定が 409 `live_lesson_quota_exceeded`、翌月には
//!   再び確定できる
//! - plus の契約は 4 回目まで確定でき、5 回目が 409 `live_lesson_quota_exceeded`（変更シナリオ cs-003）
//! - 開始の 23 時間前のキャンセルで枠が減り、25 時間前のキャンセルでは確定で使った枠が戻る

use std::time::{Duration, SystemTime};

use application::{
    identity::{TokenClaims, TokenIssuer},
    in_memory::ManualClock,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{AccountId, Role};
use infra::MockTokenService;
use serde_json::{Value, json};
use tower::ServiceExt;

const HOUR: Duration = Duration::from_secs(3600);

/// 2026-10-05T00:00:00+09:00（2026-10-04T15:00:00Z）
fn oct5_jst() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_791_126_000)
}

/// 2026-11-01T00:00:00+09:00（2026-10-31T15:00:00Z）。暦月は日本時間で区切る
fn nov1_jst() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_458_800)
}

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn post_json(path: &str, token: &str, body: Value) -> Request<Body> {
    Request::post(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap()
}

struct Scenario {
    router: Router,
    clock: ManualClock,
    coach: String,
    student: String,
}

/// 2026-10-05（日本時間）に生徒を登録し、テニスの月額契約（プランは選ばない＝standard）を開始した状態から始める。
async fn subscribed_student() -> Scenario {
    subscribed_student_with(json!({ "sport": "tennis", "payment_method_token": "tok_mock" })).await
}

/// 2026-10-05（日本時間）に生徒を登録し、`request` で月額契約を開始した状態から始める。
async fn subscribed_student_with(request: Value) -> Scenario {
    let clock = ManualClock::new(oct5_jst());
    let router = server::router_at(clock.clone());
    let coach = MockTokenService
        .issue(TokenClaims {
            account_id: AccountId::from_u128(42),
            role: Role::Coach,
        })
        .into_string();
    let (status, account) = send(
        &router,
        Request::post("/accounts")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "tennis" })
                    .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{account}");
    let student = account["access_token"].as_str().unwrap().to_string();

    let (status, body) = send(&router, post_json("/subscriptions", &student, request)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    Scenario {
        router,
        clock,
        coach,
        student,
    }
}

impl Scenario {
    /// コーチが `starts_at`（RFC 3339）から 1 時間の枠を公開し、生徒がそれを予約する（確定前）。予約 ID を返す。
    async fn book(&self, starts_at: &str, ends_at: &str) -> String {
        let (status, slot) = send(
            &self.router,
            post_json(
                "/slots",
                &self.coach,
                json!({ "sport": "tennis", "starts_at": starts_at, "ends_at": ends_at }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{slot}");
        let (status, booking) = send(
            &self.router,
            post_json(
                "/bookings",
                &self.student,
                json!({ "slot_id": slot["slot_id"], "is_trial": false }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{booking}");
        booking["booking_id"].as_str().unwrap().to_string()
    }

    async fn confirm(&self, booking_id: &str) -> (StatusCode, Value) {
        send(
            &self.router,
            post_json(
                &format!("/bookings/{booking_id}/confirm"),
                &self.student,
                json!({}),
            ),
        )
        .await
    }

    async fn cancel(&self, booking_id: &str) {
        let (status, body) = send(
            &self.router,
            post_json(
                &format!("/bookings/{booking_id}/cancel"),
                &self.student,
                json!({}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    /// `GET /entitlements/me` の `live_lesson_quota`。
    async fn quota(&self) -> Value {
        let (status, body) = send(
            &self.router,
            Request::get("/entitlements/me")
                .header(header::AUTHORIZATION, format!("Bearer {}", self.student))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["live_lesson_quota"].clone()
    }
}

/// standard の契約の `live_lesson_quota`。
fn quota(month: &str, used: u8) -> Value {
    quota_of(2, month, used)
}

/// 上限 `limit` の `live_lesson_quota`。
fn quota_of(limit: u8, month: &str, used: u8) -> Value {
    json!({ "month": month, "limit": limit, "used": used, "remaining": limit - used })
}

#[tokio::test]
async fn third_confirmation_in_the_same_month_is_rejected_and_the_next_month_allows_it_again() {
    let s = subscribed_student().await;
    assert_eq!(s.quota().await, quota("2026-10", 0));
    let first = s.book("2026-12-01T01:00:00Z", "2026-12-01T02:00:00Z").await;
    let second = s.book("2026-12-02T01:00:00Z", "2026-12-02T02:00:00Z").await;
    let third = s.book("2026-12-03T01:00:00Z", "2026-12-03T02:00:00Z").await;

    for (booking, used) in [(&first, 1), (&second, 2)] {
        let (status, body) = s.confirm(booking).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(s.quota().await, quota("2026-10", used), "確定で枠を使う");
    }

    let (status, body) = s.confirm(&third).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["code"], "live_lesson_quota_exceeded",
        "3 回目は予約できない"
    );
    assert_eq!(s.quota().await, quota("2026-10", 2));

    // 10 月 31 日 23:59（日本時間）はまだ 10 月
    s.clock.set(nov1_jst() - Duration::from_secs(60));
    let (status, _) = s.confirm(&third).await;
    assert_eq!(status, StatusCode::CONFLICT);

    // 翌月（日本時間の 11 月 1 日 0 時）には再び確定できる
    s.clock.set(nov1_jst());
    assert_eq!(s.quota().await, quota("2026-11", 0));
    let (status, body) = s.confirm(&third).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "confirmed");
    assert_eq!(s.quota().await, quota("2026-11", 1));
}

#[tokio::test]
async fn plus_confirms_up_to_the_fourth_in_the_same_month_and_rejects_the_fifth() {
    let s = subscribed_student_with(
        json!({ "sport": "tennis", "plan": "plus", "payment_method_token": "tok_mock" }),
    )
    .await;
    let (status, me) = send(
        &s.router,
        Request::get("/entitlements/me")
            .header(header::AUTHORIZATION, format!("Bearer {}", s.student))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["plan"], "plus");
    assert_eq!(s.quota().await, quota_of(4, "2026-10", 0));

    let mut bookings = Vec::new();
    for day in 1..=5 {
        bookings.push(
            s.book(
                &format!("2026-12-{day:02}T01:00:00Z"),
                &format!("2026-12-{day:02}T02:00:00Z"),
            )
            .await,
        );
    }
    for (used, booking) in (1..=4).zip(&bookings) {
        let (status, body) = s.confirm(booking).await;
        assert_eq!(status, StatusCode::OK, "{used} 回目: {body}");
        assert_eq!(s.quota().await, quota_of(4, "2026-10", used));
    }

    let (status, body) = s.confirm(&bookings[4]).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["code"], "live_lesson_quota_exceeded",
        "5 回目は予約できない"
    );
    assert_eq!(s.quota().await, quota_of(4, "2026-10", 4));

    // 翌月はプランの上限から再び数える
    s.clock.set(nov1_jst());
    assert_eq!(s.quota().await, quota_of(4, "2026-11", 0));
    let (status, body) = s.confirm(&bookings[4]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(s.quota().await, quota_of(4, "2026-11", 1));
}

#[tokio::test]
async fn cancelling_23_hours_before_consumes_the_quota_and_25_hours_before_returns_it() {
    let s = subscribed_student().await;
    // どちらも 2026-10-20T10:00:00Z（日本時間 19 時）開始と、その翌日の同じ時刻
    let starts_at = oct5_jst() + Duration::from_secs(15 * 86_400) + 19 * HOUR;
    let pending = s.book("2026-10-20T10:00:00Z", "2026-10-20T11:00:00Z").await;
    let confirmed = s.book("2026-10-21T10:00:00Z", "2026-10-21T11:00:00Z").await;
    let (status, _) = s.confirm(&confirmed).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(s.quota().await, quota("2026-10", 1));

    // 確定前の予約を開始の 23 時間前にキャンセル: 枠が減る
    s.clock.set(starts_at - 23 * HOUR);
    s.cancel(&pending).await;
    assert_eq!(
        s.quota().await,
        quota("2026-10", 2),
        "23 時間前のキャンセルは回数を消費する"
    );

    // 確定した予約を開始の 25 時間前にキャンセル: 確定で使った枠が戻る
    s.clock.set(starts_at + 24 * HOUR - 25 * HOUR);
    s.cancel(&confirmed).await;
    assert_eq!(
        s.quota().await,
        quota("2026-10", 1),
        "25 時間前のキャンセルは回数を消費しない"
    );
}

#[tokio::test]
async fn confirmed_booking_cancelled_23_hours_before_is_counted_once() {
    let s = subscribed_student().await;
    let starts_at = oct5_jst() + Duration::from_secs(15 * 86_400) + 19 * HOUR;
    let booking = s.book("2026-10-20T10:00:00Z", "2026-10-20T11:00:00Z").await;
    let (status, _) = s.confirm(&booking).await;
    assert_eq!(status, StatusCode::OK);

    s.clock.set(starts_at - 23 * HOUR);
    s.cancel(&booking).await;
    assert_eq!(
        s.quota().await,
        quota("2026-10", 1),
        "確定で使った分のまま（同じ予約で 2 回は使わない）"
    );
}
