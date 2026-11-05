//! `GET /coaches/{coach_id}/leave-preview`（admin-web の `getCoachLeavePreview`）と `POST /coaches/{coach_id}/leaves`
//! （`startCoachLeave`）の統合テスト。応答を admin-web の OpenAPI のスキーマで検査する。どちらも本部だけが呼べる（コーチ・生徒は
//! 403）。期間の予約と予約のない枠・件数・お知らせの見本が出ること、お休みにすると予約が本部のキャンセルになって枠が終了に
//! なり、キャンセルした予約ごとに `BookingCancelled`（本部・回数を消費しない）を出すこと、確かめた予約が変わった・重なる
//! お休み・期間の誤り・コーチでない ID の失敗の応答を確かめる。

use std::time::{Duration, SystemTime};

use application::{
    catalog::CatalogLessonMenuLookup,
    domain_event::DomainEvent,
    identity::{
        AccessToken, AccountRepository, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier,
    },
    in_memory::{
        FixedClock, InMemoryAccountRepository, InMemoryBookingRepository,
        InMemoryLessonMenuRepository, InMemorySlotRepository, RecordingDomainEventPublisher,
    },
    profile::{ProfileCoachAccountLookup, ProfileStudentDisplayNameLookup},
    scheduling::{BookingRepository, PreviewCoachLeave, StartCoachLeave, booking_id_uuid},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    identity::{Account, AccountId, Email, Role},
    profile::DisplayName,
    scheduling::{
        Booking, BookingCanceller, BookingId, BookingStatus, Slot, SlotCapacity, SlotId, SlotStatus,
    },
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

const HEADQUARTERS: &str = "headquarters:1";
const COACH: AccountId = AccountId::from_u128(2);
const COACH_ID: &str = "00000000-0000-0000-0000-000000000002";
const STUDENT: AccountId = AccountId::from_u128(3);
const STUDENT_ID: &str = "00000000-0000-0000-0000-000000000003";
const MINUTE: Duration = Duration::from_secs(60);

/// 2026-11-05T12:00:00+09:00（今日は 11/5）
fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_847_600)
}

/// 2026-11-10T19:00:00+09:00
fn nov10_19() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_794_304_800)
}

fn account(id: AccountId, role: Role, name: &str, sports: &[Sport]) -> Account {
    Account::new(
        id,
        Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
        DisplayName::new(name).unwrap(),
        role,
        sports.iter().copied(),
        SystemTime::UNIX_EPOCH,
    )
    .unwrap()
}

fn slot(id: u128, starts_at: SystemTime) -> Slot {
    Slot::publish(
        SlotId::from_u128(id),
        COACH,
        Sport::Tennis,
        starts_at,
        starts_at + 30 * MINUTE,
        SlotCapacity::ONE,
    )
    .unwrap()
}

struct App {
    router: Router,
    bookings: InMemoryBookingRepository,
    events: RecordingDomainEventPublisher,
    booking_id: BookingId,
}

/// 本部 1・コーチ 2（山田 健）・生徒 3（佐藤 花子）と、11/10 19:00 の枠への確定待ちの体験予約と、11/13 19:00 の予約のない枠。
async fn app() -> App {
    let accounts = InMemoryAccountRepository::default();
    for account in [
        account(AccountId::from_u128(1), Role::Headquarters, "本部", &[]),
        account(COACH, Role::Coach, "山田 健", &[Sport::Tennis]),
        account(STUDENT, Role::Student, "佐藤 花子", &[Sport::Tennis]),
    ] {
        accounts.insert(account).await.unwrap();
    }
    let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
        slot(10, nov10_19()),
        slot(11, nov10_19() + Duration::from_secs(3 * 24 * 3600)),
    ]));
    let booking_id = bookings.next_id();
    bookings
        .reserve(SlotId::from_u128(10), STUDENT, |slot, existing| {
            Booking::reserve(
                booking_id,
                STUDENT,
                true,
                slot,
                existing,
                SystemTime::UNIX_EPOCH,
            )
        })
        .await
        .unwrap();
    let events = RecordingDomainEventPublisher::default();
    let router = interface::coach_leaves_router(
        PreviewCoachLeave::new(
            bookings.clone(),
            ProfileCoachAccountLookup::new(accounts.clone()),
            ProfileStudentDisplayNameLookup::new(accounts.clone()),
            CatalogLessonMenuLookup::new(InMemoryLessonMenuRepository::default()),
            FixedClock(now()),
        ),
        StartCoachLeave::new(
            bookings.clone(),
            ProfileCoachAccountLookup::new(accounts),
            events.clone(),
            FixedClock(now()),
        ),
        FakeTokens,
    );
    App {
        router,
        bookings,
        events,
        booking_id,
    }
}

/// 呼んで、応答を admin-web の契約の `path` の操作で検査する。
async fn send(
    router: &Router,
    method: &str,
    uri: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let request = match body {
        Some(body) => request
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    support::Contract::load("admin-web").assert_response(
        &method.to_lowercase(),
        path,
        status,
        &body,
    );
    (status, body)
}

async fn preview(router: &Router, coach_id: &str, query: &str, token: &str) -> (StatusCode, Value) {
    send(
        router,
        "GET",
        &format!("/coaches/{coach_id}/leave-preview?{query}"),
        "/coaches/{coach_id}/leave-preview",
        Some(token),
        None,
    )
    .await
}

async fn start(router: &Router, body: Value) -> (StatusCode, Value) {
    send(
        router,
        "POST",
        &format!("/coaches/{COACH_ID}/leaves"),
        "/coaches/{coach_id}/leaves",
        Some(HEADQUARTERS),
        Some(body),
    )
    .await
}

#[tokio::test]
async fn the_preview_lists_the_bookings_open_slots_and_a_notification_sample() {
    let app = app().await;
    let (status, body) = preview(
        &app.router,
        COACH_ID,
        "from=2026-11-10&to=2026-11-16",
        HEADQUARTERS,
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["coach"],
        json!({
            "coach_id": COACH_ID,
            "display_name": "山田 健",
            "sports": ["tennis"],
            "acceptance_status": "accepting"
        })
    );
    assert_eq!(
        (body["from"].as_str(), body["to"].as_str()),
        (Some("2026-11-10"), Some("2026-11-16"))
    );
    assert_eq!(
        body["bookings"],
        json!([{
            "booking_id": booking_id_uuid::format(app.booking_id),
            "slot_id": "00000000-0000-0000-0000-00000000000a",
            "starts_at": "2026-11-10T10:00:00Z",
            "ends_at": "2026-11-10T10:30:00Z",
            "status": "pending",
            "is_trial": true,
            "student_id": STUDENT_ID,
            "student_display_name": "佐藤 花子"
        }])
    );
    assert_eq!(body["open_slots"].as_array().unwrap().len(), 1);
    assert_eq!(body["open_slots"][0]["capacity"], 1);
    assert_eq!(
        (body["slot_count"].as_u64(), body["student_count"].as_u64()),
        (Some(2), Some(1))
    );
    assert_eq!(
        body["notification_preview"],
        json!({
            "title": "コーチのお休みで予約をキャンセルしました",
            "body": "山田 健 コーチのお休みのため、11/10（火）19:00〜19:30 の予約を本部でキャンセルしました。今月のビデオ通話の枠は使いません。ほかの枠やコーチを探して予約できます。"
        })
    );
}

#[tokio::test]
async fn the_preview_rejects_bad_periods_unknown_coaches_and_other_roles() {
    let app = app().await;
    for query in [
        "from=2026-11-05&to=2026-11-06",
        "from=2026-11-10&to=2026-11-09",
        "from=2026-11-10&to=2026-12-11",
        "from=2026-11-10",
        "from=11/10&to=2026-11-16",
    ] {
        let (status, body) = preview(&app.router, COACH_ID, query, HEADQUARTERS).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}: {body}");
        assert_eq!(body["code"], "validation_failed");
    }
    for coach_id in [STUDENT_ID, "not-a-uuid"] {
        let (status, body) = preview(
            &app.router,
            coach_id,
            "from=2026-11-10&to=2026-11-16",
            HEADQUARTERS,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    }
    for token in ["coach:2", "student:3"] {
        let (status, _) = preview(
            &app.router,
            COACH_ID,
            "from=2026-11-10&to=2026-11-16",
            token,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
}

#[tokio::test]
async fn starting_a_leave_cancels_by_headquarters_and_withdraws_the_slots() {
    let app = app().await;
    let booking = booking_id_uuid::format(app.booking_id);

    let (status, body) = start(
        &app.router,
        json!({
            "from": "2026-11-10",
            "to": "2026-11-16",
            "note": "ご迷惑をおかけします。",
            "expected_booking_ids": [booking]
        }),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["coach_leave"]["coach_id"], COACH_ID);
    assert_eq!(body["coach_leave"]["from"], "2026-11-10");
    assert_eq!(body["coach_leave"]["to"], "2026-11-16");
    assert_eq!(body["coach_leave"]["note"], "ご迷惑をおかけします。");
    assert_eq!(
        body["coach_leave"]["created_by"],
        "00000000-0000-0000-0000-000000000001"
    );
    assert_eq!(body["cancelled_booking_ids"], json!([booking]));
    assert_eq!(
        (
            body["cancelled_booking_count"].as_u64(),
            body["notified_student_count"].as_u64(),
            body["withdrawn_slot_count"].as_u64()
        ),
        (Some(1), Some(1), Some(2))
    );
    let cancelled = app.bookings.find(app.booking_id).await.unwrap().unwrap();
    assert_eq!(cancelled.status(), BookingStatus::Cancelled);
    assert_eq!(
        cancelled.cancelled_by(),
        Some(BookingCanceller::Headquarters)
    );
    assert!(
        app.bookings
            .slots()
            .all()
            .iter()
            .all(|slot| slot.status() == SlotStatus::Closed)
    );
    let published: Vec<_> = app
        .events
        .events()
        .into_iter()
        .map(|event| match event {
            DomainEvent::BookingCancelled {
                consumes_quota,
                cancelled_by,
                ..
            } => (consumes_quota, cancelled_by),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(published, vec![(false, BookingCanceller::Headquarters)]);

    // 同じ期間の 2 回目は重なるお休み
    let (status, body) = start(
        &app.router,
        json!({"from": "2026-11-12", "to": "2026-11-20", "expected_booking_ids": []}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "coach_leave_overlaps");
}

#[tokio::test]
async fn a_changed_period_and_a_bad_request_change_nothing() {
    let app = app().await;
    for (body, status, code) in [
        (
            json!({"from": "2026-11-10", "to": "2026-11-16", "expected_booking_ids": []}),
            StatusCode::CONFLICT,
            "leave_preview_outdated",
        ),
        (
            json!({"from": "2026-11-10", "to": "2026-11-16", "note": "あ".repeat(201),
                   "expected_booking_ids": [booking_id_uuid::format(app.booking_id)]}),
            StatusCode::UNPROCESSABLE_ENTITY,
            "validation_failed",
        ),
        (
            json!({"from": "2026-11-05", "to": "2026-11-16", "expected_booking_ids": []}),
            StatusCode::UNPROCESSABLE_ENTITY,
            "validation_failed",
        ),
        (
            json!({"from": "2026-11-10", "to": "2026-11-16", "expected_booking_ids": ["x"]}),
            StatusCode::UNPROCESSABLE_ENTITY,
            "validation_failed",
        ),
    ] {
        let (actual, response) = start(&app.router, body).await;
        assert_eq!(
            (actual, response["code"].as_str()),
            (status, Some(code)),
            "{response}"
        );
    }
    assert_eq!(
        app.bookings
            .find(app.booking_id)
            .await
            .unwrap()
            .unwrap()
            .status(),
        BookingStatus::Pending
    );
    assert!(app.bookings.coach_leaves().is_empty());
    assert!(app.events.events().is_empty());
}
