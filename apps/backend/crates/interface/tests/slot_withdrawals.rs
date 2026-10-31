//! `/coaches/me/slots/{slot_id}/withdraw` の統合テスト。admin-web の `POST /coaches/me/slots/{slot_id}/withdraw`
//! （`withdrawMySlot`）のレスポンスの形を OpenAPI のスキーマで検査し、予約の入っていない・まだ始まっていない公開中の
//! 自分の枠だけを閉じて返すこと（200）・行動イベント `slot.withdrawn` を 1 回発火すること・他のコーチの枠とない枠は 404、
//! 予約が入っている・始まった・すでに閉じた枠は 409（`code` で分ける）で何も変えないこと・コーチ以外（生徒・本部）は 403、
//! トークンがなければ 401 になることを確かめる。

use std::time::{Duration, SystemTime};

use application::{
    analytics::AnalyticsEvent,
    identity::{AccessToken, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier},
    in_memory::{FixedClock, InMemorySlotRepository, RecordingAnalyticsSink},
    scheduling::{WithdrawSlot, slot_id_uuid},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    identity::{AccountId, Role},
    scheduling::{Slot, SlotCapacity, SlotId, SlotSeats, SlotStatus},
};
use serde_json::{Value, json};
use tower::ServiceExt;

mod support;

/// `<役割>:<ID の数値>` の形のトークンを通す偽物（interface は infra のモックに依存できない）。
struct FakeTokens;

impl TokenIssuer for FakeTokens {
    fn issue(&self, _claims: TokenClaims) -> AccessToken {
        unreachable!("このテストでは発行しない")
    }
}

impl TokenVerifier for FakeTokens {
    fn verify(&self, token: &str) -> Result<TokenClaims, InvalidToken> {
        let (role, id) = token.split_once(':').ok_or(InvalidToken)?;
        let role = match role {
            "student" => Role::Student,
            "coach" => Role::Coach,
            "headquarters" => Role::Headquarters,
            _ => return Err(InvalidToken),
        };
        let id = id.parse().map_err(|_| InvalidToken)?;
        Ok(TokenClaims {
            account_id: AccountId::from_u128(id),
            role,
        })
    }
}

const COACH: &str = "coach:9";
const OTHER_COACH: &str = "coach:8";
const STUDENT: &str = "student:1";
const HEADQUARTERS: &str = "headquarters:5";

fn account_id(token: &str) -> AccountId {
    AccountId::from_u128(token.split_once(':').unwrap().1.parse().unwrap())
}

/// 2026-10-26T00:00:00Z から `hour` 時間後
fn at(hour: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800 + hour * 3600)
}

/// 今（取り下げる時刻）
const NOW_HOUR: u64 = 5;

/// 公開中で予約のない枠
const OPEN: u128 = 1;
/// 公開中で未確定の予約が 1 件ある枠（定員 2）
const PENDING: u128 = 2;
/// 確定した予約で埋まった枠
const BOOKED: u128 = 3;
/// 公開中で予約がないが、もう始まった枠
const STARTED: u128 = 4;
/// すでに閉じた枠
const CLOSED: u128 = 5;
/// 他のコーチの公開中の枠
const OTHERS: u128 = 6;

fn slot(
    id: u128,
    coach: &str,
    from: u64,
    status: SlotStatus,
    capacity: i64,
    (pending, confirmed): (u32, u32),
) -> Slot {
    Slot::restore(
        SlotId::from_u128(id),
        account_id(coach),
        Sport::Tennis,
        at(from),
        at(from + 1),
        status,
        SlotSeats::restore(SlotCapacity::new(capacity).unwrap(), pending, confirmed).unwrap(),
    )
    .unwrap()
}

struct App {
    router: Router,
    slots: InMemorySlotRepository,
    analytics: RecordingAnalyticsSink,
}

fn app() -> App {
    let slots = InMemorySlotRepository::with_slots([
        slot(OPEN, COACH, 10, SlotStatus::Open, 1, (0, 0)),
        slot(PENDING, COACH, 12, SlotStatus::Open, 2, (1, 0)),
        slot(BOOKED, COACH, 14, SlotStatus::Booked, 1, (0, 1)),
        slot(STARTED, COACH, NOW_HOUR, SlotStatus::Open, 1, (0, 0)),
        slot(CLOSED, COACH, 16, SlotStatus::Closed, 1, (0, 0)),
        slot(OTHERS, OTHER_COACH, 10, SlotStatus::Open, 1, (0, 0)),
    ]);
    let analytics = RecordingAnalyticsSink::default();
    let router = interface::slot_withdrawal_router(
        WithdrawSlot::new(slots.clone(), analytics.clone(), FixedClock(at(NOW_HOUR))),
        FakeTokens,
    );
    App {
        router,
        slots,
        analytics,
    }
}

fn slot_id(id: u128) -> String {
    slot_id_uuid::format(SlotId::from_u128(id))
}

async fn withdraw(router: &Router, slot_id: &str, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::post(format!("/coaches/me/slots/{slot_id}/withdraw"));
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap();
    support::Contract::load("admin-web").assert_response(
        "post",
        "/coaches/me/slots/{slot_id}/withdraw",
        status,
        &body,
    );
    (status, body)
}

fn status_of(app: &App, id: u128) -> SlotStatus {
    app.slots
        .all()
        .into_iter()
        .find(|slot| slot.id() == SlotId::from_u128(id))
        .unwrap()
        .status()
}

#[tokio::test]
async fn a_coach_withdraws_an_open_slot_without_bookings() {
    let app = app();

    let (status, body) = withdraw(&app.router, &slot_id(OPEN), Some(COACH)).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "slot_id": slot_id(OPEN),
            "coach_id": application::identity::account_id_uuid::format(account_id(COACH)),
            "sport": "tennis",
            "starts_at": "2026-10-26T10:00:00Z",
            "ends_at": "2026-10-26T11:00:00Z",
            "status": "closed",
            "capacity": 1,
            "remaining_seats": 1
        })
    );
    assert_eq!(status_of(&app, OPEN), SlotStatus::Closed);
    assert_eq!(
        app.analytics.events(),
        vec![AnalyticsEvent::SlotWithdrawn {
            coach_id: account_id(COACH),
            slot_id: SlotId::from_u128(OPEN),
            starts_at: at(10),
            occurred_at: at(NOW_HOUR),
        }]
    );

    // 2 回目はすでに閉じた枠
    let (status, body) = withdraw(&app.router, &slot_id(OPEN), Some(COACH)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "slot_already_closed");
    assert_eq!(app.analytics.events().len(), 1);
}

#[tokio::test]
async fn slots_with_bookings_started_or_closed_are_409_and_unchanged() {
    let app = app();
    for (id, code) in [
        (PENDING, "slot_has_active_booking"),
        (BOOKED, "slot_has_active_booking"),
        (STARTED, "slot_already_started"),
        (CLOSED, "slot_already_closed"),
    ] {
        let before = status_of(&app, id);
        let (status, body) = withdraw(&app.router, &slot_id(id), Some(COACH)).await;
        assert_eq!(status, StatusCode::CONFLICT, "{id}: {body}");
        assert_eq!(body["code"], code, "{id}");
        assert_eq!(status_of(&app, id), before, "{id}");
    }
    assert_eq!(app.analytics.events(), vec![]);
}

#[tokio::test]
async fn another_coachs_slot_unknown_slot_and_non_uuid_are_404() {
    let app = app();
    for id in [slot_id(OTHERS), slot_id(99), "not-a-uuid".to_string()] {
        let (status, body) = withdraw(&app.router, &id, Some(COACH)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}: {body}");
        assert_eq!(body["code"], "not_found");
    }
    assert_eq!(status_of(&app, OTHERS), SlotStatus::Open);
    assert_eq!(app.analytics.events(), vec![]);
}

#[tokio::test]
async fn only_coaches_can_withdraw() {
    let app = app();
    for token in [STUDENT, HEADQUARTERS] {
        let (status, body) = withdraw(&app.router, &slot_id(OPEN), Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
    }
    let (status, body) = withdraw(&app.router, &slot_id(OPEN), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(status_of(&app, OPEN), SlotStatus::Open);
    assert_eq!(app.analytics.events(), vec![]);
}
