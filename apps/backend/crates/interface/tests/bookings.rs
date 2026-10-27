//! `/bookings` の統合テスト。student-app の `POST /bookings`（`createBooking`）と
//! `POST /bookings/{booking_id}/confirm`（`confirmBooking`）・`POST /bookings/{booking_id}/cancel`（`cancelBooking`）の
//! レスポンスの形を OpenAPI のスキーマで検査し、二重予約の拒否・体験予約の 1 回限り・確定でスロットが埋まること・
//! キャンセルでスロットが空くこととキャンセル規定の判定（`consumes_quota`）・発火するイベントを確かめる。

use std::time::{Duration, SystemTime};

use application::{
    analytics::AnalyticsEvent,
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo},
    domain_event::DomainEvent,
    entitlement::{EntitlementQuotaGate, EntitlementReviewGate, GetEntitlement, StartSubscription},
    identity::{
        AccessToken, InvalidToken, RegisterAccount, TokenClaims, TokenIssuer, TokenVerifier,
        account_id_uuid,
    },
    in_memory::{
        FixedClock, FixedVideoCallProvider, FixedVideoStorage, InMemoryAccountRepository,
        InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemoryLessonCompletionRepository, InMemoryLiveLessonSessionRepository,
        InMemorySlotRepository, InMemoryVideoRepository, RecordingAnalyticsSink,
        RecordingDomainEventPublisher,
    },
    lesson::RecordLessonCompletion,
    live_lesson::{EndLiveLessonSession, StartLiveLessonSession},
    scheduling::{
        CancelBooking, ConfirmBooking, CreateBooking, GetSlotSupply, PublishSlot,
        SchedulingBookingLookup, SearchSlots, booking_id_uuid, slot_id_uuid,
    },
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    entitlement::{Entitlement, EntitlementId, LiveLessonQuota, SubscriptionId, YearMonth},
    identity::{AccountId, Role, Sport},
    scheduling::{BookingId, Slot, SlotId, SlotStatus},
};
use serde_json::{Value, json};
use tower::ServiceExt;

mod support;

/// 2026-10-26T00:00:00Z（現在時刻）
const NOW: Duration = Duration::from_secs(1_792_972_800);

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

const STUDENT: &str = "student:1";
const OTHER_STUDENT: &str = "student:2";
const COACH: AccountId = AccountId::from_u128(9);

fn account_id(token: &str) -> AccountId {
    AccountId::from_u128(token.split_once(':').unwrap().1.parse().unwrap())
}

/// 2026-11-01 の `hour` 時（UTC）。
fn nov1(hour: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200 + hour * 3600)
}

/// スロット `id` は 2026-11-01 の `id` 時から 1 時間。
fn slot(id: u128, status: SlotStatus) -> Slot {
    let hour = id as u64;
    Slot::restore(
        SlotId::from_u128(id),
        COACH,
        Sport::Tennis,
        nov1(hour),
        nov1(hour + 1),
        status,
    )
    .unwrap()
}

fn slot_id(id: u128) -> String {
    slot_id_uuid::format(SlotId::from_u128(id))
}

struct App {
    router: Router,
    slots: InMemorySlotRepository,
    analytics: RecordingAnalyticsSink,
    events: RecordingDomainEventPublisher,
}

/// スロット 10・11・12 は公開中、13 は閉じている。
fn app() -> App {
    app_with([])
}

/// [`app`] のスロットに `extra` を足す。
fn app_with(extra: impl IntoIterator<Item = Slot>) -> App {
    app_with_entitlements(extra, InMemoryEntitlementRepository::default())
}

/// [`app_with`] の受講権の保存先を `entitlements` にする。
fn app_with_entitlements(
    extra: impl IntoIterator<Item = Slot>,
    entitlements: InMemoryEntitlementRepository,
) -> App {
    let analytics = RecordingAnalyticsSink::default();
    let events = RecordingDomainEventPublisher::default();
    let clock = FixedClock(SystemTime::UNIX_EPOCH + NOW);
    let slots = InMemorySlotRepository::with_slots(
        [
            slot(10, SlotStatus::Open),
            slot(11, SlotStatus::Open),
            slot(12, SlotStatus::Open),
            slot(13, SlotStatus::Closed),
        ]
        .into_iter()
        .chain(extra),
    );
    let bookings = InMemoryBookingRepository::new(slots.clone());
    let router = interface::router_with(
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            analytics.clone(),
            clock,
        ),
        PublishSlot::new(slots.clone(), analytics.clone(), clock),
        SearchSlots::new(slots.clone(), clock),
        CreateBooking::new(bookings.clone(), analytics.clone(), clock),
        ConfirmBooking::new(
            bookings.clone(),
            EntitlementQuotaGate::new(entitlements.clone()),
            analytics.clone(),
            events.clone(),
            clock,
        ),
        CancelBooking::new(bookings, analytics.clone(), events.clone(), clock),
        GetSlotSupply::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            clock,
        ),
        StartSubscription::new(
            entitlements.clone(),
            analytics.clone(),
            events.clone(),
            clock,
        ),
        GetEntitlement::new(entitlements, clock),
        StartLiveLessonSession::new(
            InMemoryLiveLessonSessionRepository::default(),
            SchedulingBookingLookup::new(InMemoryBookingRepository::new(
                InMemorySlotRepository::default(),
            )),
            FixedVideoCallProvider::default(),
            RecordingAnalyticsSink::default(),
            clock,
        ),
        EndLiveLessonSession::new(
            InMemoryLiveLessonSessionRepository::default(),
            FixedVideoCallProvider::default(),
            RecordingAnalyticsSink::default(),
            clock,
        ),
        SubmitVideo::new(
            InMemoryVideoRepository::default(),
            FixedVideoStorage,
            RecordingAnalyticsSink::default(),
            clock,
        ),
        GetVideoReview::new(InMemoryVideoRepository::default()),
        ReturnVideoReview::new(
            InMemoryVideoRepository::default(),
            EntitlementReviewGate,
            RecordingAnalyticsSink::default(),
            clock,
        ),
        RecordLessonCompletion::new(
            InMemoryLiveLessonSessionRepository::default(),
            InMemoryVideoRepository::default(),
            InMemoryLessonCompletionRepository::default(),
            RecordingAnalyticsSink::default(),
            RecordingDomainEventPublisher::default(),
            clock,
        ),
        application::messaging::ListMessageThreads::new(
            application::in_memory::InMemoryMessageThreadRepository::default(),
        ),
        application::messaging::ListMessages::new(
            application::in_memory::InMemoryMessageThreadRepository::default(),
        ),
        application::messaging::SendMessage::new(
            application::in_memory::InMemoryMessageThreadRepository::default(),
            application::entitlement::EntitlementChatGate::new(
                application::in_memory::InMemoryEntitlementRepository::default(),
            ),
            RecordingAnalyticsSink::default(),
            clock,
        ),
        application::messaging::MarkMessageThreadRead::new(
            application::in_memory::InMemoryMessageThreadRepository::default(),
            clock,
        ),
        FakeTokens,
    );
    App {
        router,
        slots,
        analytics,
        events,
    }
}

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn post_bookings(router: &Router, token: Option<&str>, body: Value) -> (StatusCode, Value) {
    let mut request = Request::post("/bookings").header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    send(router, request.body(Body::from(body.to_string())).unwrap()).await
}

async fn post_confirm(
    router: &Router,
    token: Option<&str>,
    booking_id: &str,
) -> (StatusCode, Value) {
    let mut request = Request::post(format!("/bookings/{booking_id}/confirm"));
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    send(router, request.body(Body::empty()).unwrap()).await
}

fn booking(slot: u128, is_trial: bool) -> Value {
    json!({ "slot_id": slot_id(slot), "is_trial": is_trial })
}

/// 生徒 `token` がスロット `slot` を予約し、booking_id を返す。
async fn create(router: &Router, token: &str, slot: u128, is_trial: bool) -> String {
    let (status, body) = post_bookings(router, Some(token), booking(slot, is_trial)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["booking_id"].as_str().unwrap().to_string()
}

fn assert_create_contract(status: StatusCode, body: &Value) {
    support::Contract::load("student-app").assert_response("post", "/bookings", status, body);
}

fn assert_confirm_contract(status: StatusCode, body: &Value) {
    support::Contract::load("student-app").assert_response(
        "post",
        "/bookings/{booking_id}/confirm",
        status,
        body,
    );
}

fn slot_status(app: &App, id: u128) -> SlotStatus {
    app.slots
        .all()
        .into_iter()
        .find(|s| s.id() == SlotId::from_u128(id))
        .unwrap()
        .status()
}

// --- POST /bookings ---

#[tokio::test]
async fn student_creates_a_trial_booking_with_the_contract_shape() {
    let app = app();

    let (status, body) = post_bookings(&app.router, Some(STUDENT), booking(10, true)).await;

    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_create_contract(status, &body);
    assert_eq!(body["slot_id"], slot_id(10));
    assert_eq!(
        body["student_id"],
        account_id_uuid::format(account_id(STUDENT)),
        "予約する生徒はトークンのアカウント"
    );
    assert_eq!(body["coach_id"], account_id_uuid::format(COACH));
    assert_eq!(body["is_trial"], true);
    assert_eq!(body["status"], "pending");
    assert_eq!(body["starts_at"], "2026-11-01T10:00:00Z");
    assert_eq!(body["created_at"], "2026-10-26T00:00:00Z");
    assert_eq!(body["confirmed_at"], Value::Null);
    assert_eq!(
        slot_status(&app, 10),
        SlotStatus::Open,
        "作成ではスロットを埋めない"
    );

    let events = app.analytics.events();
    assert_eq!(events.len(), 1, "booking.created は 1 回だけ発火する");
    assert_eq!(events[0].name(), "booking.created");
    let AnalyticsEvent::BookingCreated {
        student_id,
        coach_id,
        booking_id,
        slot_id: event_slot,
        is_trial,
        occurred_at,
    } = &events[0]
    else {
        panic!("{:?}", events[0]);
    };
    assert_eq!(*student_id, account_id(STUDENT));
    assert_eq!(*coach_id, COACH);
    assert_eq!(booking_id_uuid::format(*booking_id), body["booking_id"]);
    assert_eq!(*event_slot, SlotId::from_u128(10));
    assert!(*is_trial);
    assert_eq!(*occurred_at, SystemTime::UNIX_EPOCH + NOW);
    assert!(
        app.events.events().is_empty(),
        "作成では BookingConfirmed を発行しない"
    );
}

#[tokio::test]
async fn double_booking_is_409_slot_already_booked_without_emitting() {
    let app = app();
    create(&app.router, STUDENT, 10, false).await;

    for token in [STUDENT, OTHER_STUDENT] {
        let (status, body) = post_bookings(&app.router, Some(token), booking(10, false)).await;
        assert_eq!(status, StatusCode::CONFLICT, "{token}: {body}");
        assert_create_contract(status, &body);
        assert_eq!(body["code"], "slot_already_booked");
    }
    assert_eq!(app.analytics.events().len(), 1);
}

#[tokio::test]
async fn confirmed_slot_cannot_be_booked() {
    let app = app();
    let booking_id = create(&app.router, STUDENT, 10, false).await;
    post_confirm(&app.router, Some(STUDENT), &booking_id).await;

    let (status, body) = post_bookings(&app.router, Some(OTHER_STUDENT), booking(10, false)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "slot_already_booked");
}

#[tokio::test]
async fn trial_booking_is_only_once_per_student() {
    let app = app();
    create(&app.router, STUDENT, 10, true).await;

    let (status, body) = post_bookings(&app.router, Some(STUDENT), booking(11, true)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_create_contract(status, &body);
    assert_eq!(body["code"], "trial_already_used");

    // 2 件目でも通常の予約はできる。別の生徒は初回なので体験予約できる
    create(&app.router, STUDENT, 11, false).await;
    create(&app.router, OTHER_STUDENT, 12, true).await;
    let trials: Vec<bool> = app
        .analytics
        .events()
        .iter()
        .map(|event| match event {
            AnalyticsEvent::BookingCreated { is_trial, .. } => *is_trial,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(trials, [true, false, true]);
}

#[tokio::test]
async fn trial_after_a_regular_booking_is_409() {
    let app = app();
    create(&app.router, STUDENT, 10, false).await;
    let (status, body) = post_bookings(&app.router, Some(STUDENT), booking(11, true)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "trial_already_used");
}

#[tokio::test]
async fn missing_or_closed_slot_is_404() {
    let app = app();
    for slot in [13, 99] {
        let (status, body) = post_bookings(&app.router, Some(STUDENT), booking(slot, false)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{slot}: {body}");
        assert_create_contract(status, &body);
        assert_eq!(body["code"], "not_found");
    }
    assert!(app.analytics.events().is_empty());
}

#[tokio::test]
async fn invalid_request_is_422_validation_failed() {
    let app = app();
    for body in [
        json!({ "slot_id": "not-a-uuid", "is_trial": false }),
        json!({ "slot_id": slot_id(10) }),
        json!({ "slot_id": slot_id(10), "is_trial": "yes" }),
        json!({ "is_trial": false }),
    ] {
        let (status, response) = post_bookings(&app.router, Some(STUDENT), body.clone()).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{body}: {response}"
        );
        assert_create_contract(status, &response);
        assert_eq!(response["code"], "validation_failed", "{body}");
    }
    assert!(app.analytics.events().is_empty());
}

#[tokio::test]
async fn only_students_can_book_and_confirm() {
    let app = app();
    let booking_id = create(&app.router, STUDENT, 10, false).await;
    for token in ["coach:9", "headquarters:5"] {
        let (status, body) = post_bookings(&app.router, Some(token), booking(11, false)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_create_contract(status, &body);
        assert_eq!(body["code"], "forbidden");

        let (status, body) = post_confirm(&app.router, Some(token), &booking_id).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_confirm_contract(status, &body);
    }
    assert_eq!(app.analytics.events().len(), 1);
}

#[tokio::test]
async fn without_a_valid_token_is_401() {
    let app = app();
    for token in [None, Some("unknown")] {
        let (status, body) = post_bookings(&app.router, token, booking(10, false)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}: {body}");
        assert_create_contract(status, &body);

        let (status, body) = post_confirm(&app.router, token, &slot_id(10)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}: {body}");
        assert_confirm_contract(status, &body);
    }
}

// --- POST /bookings/{booking_id}/confirm ---

#[tokio::test]
async fn confirming_books_the_slot_and_publishes_booking_confirmed() {
    let app = app();
    let booking_id = create(&app.router, STUDENT, 10, true).await;

    let (status, body) = post_confirm(&app.router, Some(STUDENT), &booking_id).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_confirm_contract(status, &body);
    assert_eq!(body["booking_id"], booking_id);
    assert_eq!(body["status"], "confirmed");
    assert_eq!(body["is_trial"], true);
    assert_eq!(body["confirmed_at"], "2026-10-26T00:00:00Z");
    assert_eq!(
        slot_status(&app, 10),
        SlotStatus::Booked,
        "確定でスロットが埋まる"
    );

    // 予約済みのスロットは生徒の検索に出ない
    let request = Request::get("/slots?sport=tennis")
        .header(header::AUTHORIZATION, format!("Bearer {STUDENT}"))
        .body(Body::empty())
        .unwrap();
    let (_, page) = send(&app.router, request).await;
    let ids: Vec<&str> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["slot_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [slot_id(11), slot_id(12)]);

    let events = app.events.events();
    assert_eq!(events.len(), 1, "BookingConfirmed は 1 回だけ発行する");
    let DomainEvent::BookingConfirmed {
        event_id,
        occurred_at,
        student_id,
        coach_id,
        booking_id: event_booking,
    } = &events[0]
    else {
        panic!("{:?}", events[0]);
    };
    assert!(!event_id.is_nil());
    assert_eq!(*occurred_at, SystemTime::UNIX_EPOCH + NOW);
    assert_eq!(*student_id, account_id(STUDENT));
    assert_eq!(*coach_id, COACH);
    assert_eq!(booking_id_uuid::format(*event_booking), booking_id);

    let analytics = app.analytics.events();
    assert_eq!(
        analytics
            .iter()
            .map(AnalyticsEvent::name)
            .collect::<Vec<_>>(),
        ["booking.created", "booking.confirmed"]
    );
    let AnalyticsEvent::BookingConfirmed {
        student_id,
        coach_id,
        booking_id: analytics_booking,
        slot_id: analytics_slot,
        occurred_at,
    } = &analytics[1]
    else {
        panic!("{:?}", analytics[1]);
    };
    assert_eq!(
        (*student_id, *coach_id, *analytics_slot, *occurred_at),
        (
            account_id(STUDENT),
            COACH,
            SlotId::from_u128(10),
            SystemTime::UNIX_EPOCH + NOW
        )
    );
    assert_eq!(booking_id_uuid::format(*analytics_booking), booking_id);
}

#[tokio::test]
async fn confirming_twice_is_409_invalid_booking_state() {
    let app = app();
    let booking_id = create(&app.router, STUDENT, 10, false).await;
    let (status, _) = post_confirm(&app.router, Some(STUDENT), &booking_id).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = post_confirm(&app.router, Some(STUDENT), &booking_id).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_confirm_contract(status, &body);
    assert_eq!(body["code"], "invalid_booking_state");
    assert_eq!(app.events.events().len(), 1);
    assert_eq!(app.analytics.events().len(), 2);
}

#[tokio::test]
async fn missing_or_another_students_booking_is_404() {
    let app = app();
    let booking_id = create(&app.router, STUDENT, 10, false).await;

    for (token, id) in [
        (OTHER_STUDENT, booking_id.as_str()),
        (STUDENT, "00000000-0000-4000-8000-000000000000"),
        (STUDENT, "not-a-uuid"),
    ] {
        let (status, body) = post_confirm(&app.router, Some(token), id).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{token} {id}: {body}");
        assert_confirm_contract(status, &body);
        assert_eq!(body["code"], "not_found");
    }
    assert_eq!(slot_status(&app, 10), SlotStatus::Open);
    assert!(app.events.events().is_empty());
}

#[tokio::test]
async fn confirming_without_live_lesson_quota_left_is_409_live_lesson_quota_exceeded() {
    // 今の暦月（2026-10）の枠を使い切った月額の生徒
    let used_up = Entitlement::monthly(
        EntitlementId::from_u128(30),
        account_id(STUDENT),
        Sport::Tennis,
        SubscriptionId::from_u128(40),
        LiveLessonQuota::restore(
            YearMonth::new(2026, 10).unwrap(),
            [BookingId::from_u128(1), BookingId::from_u128(2)],
        )
        .unwrap(),
    )
    .unwrap();
    let app = app_with_entitlements(
        [],
        InMemoryEntitlementRepository::with_entitlements([used_up]),
    );
    let booking_id = create(&app.router, STUDENT, 10, false).await;

    let (status, body) = post_confirm(&app.router, Some(STUDENT), &booking_id).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_confirm_contract(status, &body);
    assert_eq!(body["code"], "live_lesson_quota_exceeded");
    assert_eq!(
        slot_status(&app, 10),
        SlotStatus::Open,
        "確定しないのでスロットは空いたまま"
    );
    assert!(app.events.events().is_empty());

    // 枠の制約は月額の生徒だけ。受講権のない生徒は確定できる
    let other = create(&app.router, OTHER_STUDENT, 11, false).await;
    let (status, body) = post_confirm(&app.router, Some(OTHER_STUDENT), &other).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

// --- POST /bookings/{booking_id}/cancel ---

/// スロット `id` は現在時刻の `hours` 時間後から 1 時間（公開中）。
fn slot_starting_in(id: u128, hours: u64) -> Slot {
    let starts_at = SystemTime::UNIX_EPOCH + NOW + Duration::from_secs(hours * 3600);
    Slot::restore(
        SlotId::from_u128(id),
        COACH,
        Sport::Tennis,
        starts_at,
        starts_at + Duration::from_secs(3600),
        SlotStatus::Open,
    )
    .unwrap()
}

async fn post_cancel(
    router: &Router,
    token: Option<&str>,
    booking_id: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::post(format!("/bookings/{booking_id}/cancel"));
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let body = match body {
        Some(body) => {
            request = request.header(header::CONTENT_TYPE, "application/json");
            Body::from(body.to_string())
        }
        None => Body::empty(),
    };
    send(router, request.body(body).unwrap()).await
}

fn assert_cancel_contract(status: StatusCode, body: &Value) {
    support::Contract::load("student-app").assert_response(
        "post",
        "/bookings/{booking_id}/cancel",
        status,
        body,
    );
}

/// 発行した `BookingCancelled` の `consumes_quota`（1 件だけのはず）。
fn cancelled_consumes_quota(app: &App) -> bool {
    let cancelled: Vec<bool> = app
        .events
        .events()
        .iter()
        .filter_map(|event| match event {
            DomainEvent::BookingCancelled { consumes_quota, .. } => Some(*consumes_quota),
            _ => None,
        })
        .collect();
    assert_eq!(cancelled.len(), 1, "BookingCancelled は 1 回だけ発行する");
    cancelled[0]
}

#[tokio::test]
async fn cancelling_a_confirmed_booking_23_hours_before_reopens_the_slot_and_consumes_quota() {
    let app = app_with([slot_starting_in(23, 23)]);
    let booking_id = create(&app.router, STUDENT, 23, false).await;
    post_confirm(&app.router, Some(STUDENT), &booking_id).await;
    assert_eq!(slot_status(&app, 23), SlotStatus::Booked);

    let (status, body) = post_cancel(&app.router, Some(STUDENT), &booking_id, None).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_cancel_contract(status, &body);
    assert_eq!(body["booking_id"], booking_id);
    assert_eq!(body["status"], "cancelled");
    assert_eq!(body["cancelled_at"], "2026-10-26T00:00:00Z");
    assert_eq!(body["confirmed_at"], "2026-10-26T00:00:00Z");
    assert!(
        body.get("consumes_quota").is_none(),
        "回数を消費したかはレスポンスに含めない"
    );
    assert_eq!(
        slot_status(&app, 23),
        SlotStatus::Open,
        "キャンセルでスロットが公開中に戻る"
    );

    // 空いたスロットは別の生徒が予約できる
    create(&app.router, OTHER_STUDENT, 23, false).await;

    let events = app.events.events();
    assert_eq!(
        events.iter().map(DomainEvent::name).collect::<Vec<_>>(),
        ["BookingConfirmed", "BookingCancelled"]
    );
    let DomainEvent::BookingCancelled {
        event_id,
        occurred_at,
        student_id,
        coach_id,
        booking_id: event_booking,
        consumes_quota,
        cancelled_at,
    } = &events[1]
    else {
        panic!("{:?}", events[1]);
    };
    assert!(!event_id.is_nil());
    assert_eq!(*occurred_at, SystemTime::UNIX_EPOCH + NOW);
    assert_eq!(*cancelled_at, SystemTime::UNIX_EPOCH + NOW);
    assert_eq!(*student_id, account_id(STUDENT));
    assert_eq!(*coach_id, COACH);
    assert_eq!(booking_id_uuid::format(*event_booking), booking_id);
    assert!(
        *consumes_quota,
        "開始の 23 時間前のキャンセルは回数を消費する"
    );

    let cancelled: Vec<AnalyticsEvent> = app
        .analytics
        .events()
        .into_iter()
        .filter(|event| event.name() == "booking.cancelled")
        .collect();
    assert_eq!(cancelled.len(), 1, "booking.cancelled は 1 回だけ発火する");
    let AnalyticsEvent::BookingCancelled {
        student_id,
        coach_id,
        booking_id: analytics_booking,
        slot_id: analytics_slot,
        hours_before_start,
        occurred_at,
    } = &cancelled[0]
    else {
        panic!("{:?}", cancelled[0]);
    };
    assert_eq!(
        (
            *student_id,
            *coach_id,
            *analytics_slot,
            *hours_before_start,
            *occurred_at
        ),
        (
            account_id(STUDENT),
            COACH,
            SlotId::from_u128(23),
            23,
            SystemTime::UNIX_EPOCH + NOW
        )
    );
    assert_eq!(booking_id_uuid::format(*analytics_booking), booking_id);
}

#[tokio::test]
async fn cancelling_25_hours_before_does_not_consume_quota() {
    let app = app_with([slot_starting_in(25, 25)]);
    let booking_id = create(&app.router, STUDENT, 25, false).await;
    post_confirm(&app.router, Some(STUDENT), &booking_id).await;

    let (status, body) = post_cancel(
        &app.router,
        Some(STUDENT),
        &booking_id,
        Some(json!({ "reason": "都合が悪くなった" })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_cancel_contract(status, &body);
    assert!(
        !cancelled_consumes_quota(&app),
        "開始の 25 時間前のキャンセルは回数を消費しない"
    );
    let hours: Vec<i64> = app
        .analytics
        .events()
        .iter()
        .filter_map(|event| match event {
            AnalyticsEvent::BookingCancelled {
                hours_before_start, ..
            } => Some(*hours_before_start),
            _ => None,
        })
        .collect();
    assert_eq!(hours, [25]);
}

#[tokio::test]
async fn cancelling_a_pending_booking_keeps_the_slot_open() {
    let app = app_with([slot_starting_in(23, 23)]);
    let booking_id = create(&app.router, STUDENT, 23, true).await;

    let (status, body) = post_cancel(&app.router, Some(STUDENT), &booking_id, None).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_cancel_contract(status, &body);
    assert_eq!(body["status"], "cancelled");
    assert_eq!(body["confirmed_at"], Value::Null);
    assert_eq!(slot_status(&app, 23), SlotStatus::Open);
    assert!(cancelled_consumes_quota(&app));
    // キャンセルした体験予約は初回に数えないので、もう一度体験予約できる
    create(&app.router, STUDENT, 23, true).await;
}

#[tokio::test]
async fn cancelling_twice_is_409_invalid_booking_state() {
    let app = app();
    let booking_id = create(&app.router, STUDENT, 10, false).await;
    let (status, _) = post_cancel(&app.router, Some(STUDENT), &booking_id, None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = post_cancel(&app.router, Some(STUDENT), &booking_id, None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_cancel_contract(status, &body);
    assert_eq!(body["code"], "invalid_booking_state");
    assert_eq!(app.events.events().len(), 1);

    // キャンセルした予約は確定できない
    let (status, body) = post_confirm(&app.router, Some(STUDENT), &booking_id).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "invalid_booking_state");
}

#[tokio::test]
async fn cancelling_missing_or_another_students_booking_is_404() {
    let app = app();
    let booking_id = create(&app.router, STUDENT, 10, false).await;
    post_confirm(&app.router, Some(STUDENT), &booking_id).await;

    for (token, id) in [
        (OTHER_STUDENT, booking_id.as_str()),
        (STUDENT, "00000000-0000-4000-8000-000000000000"),
        (STUDENT, "not-a-uuid"),
    ] {
        let (status, body) = post_cancel(&app.router, Some(token), id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{token} {id}: {body}");
        assert_cancel_contract(status, &body);
        assert_eq!(body["code"], "not_found");
    }
    assert_eq!(slot_status(&app, 10), SlotStatus::Booked);
    assert_eq!(app.events.events().len(), 1, "BookingConfirmed だけ");
}

#[tokio::test]
async fn only_authenticated_students_can_cancel() {
    let app = app();
    let booking_id = create(&app.router, STUDENT, 10, false).await;
    for token in ["coach:9", "headquarters:5"] {
        let (status, body) = post_cancel(&app.router, Some(token), &booking_id, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_cancel_contract(status, &body);
        assert_eq!(body["code"], "forbidden");
    }
    for token in [None, Some("unknown")] {
        let (status, body) = post_cancel(&app.router, token, &booking_id, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}: {body}");
        assert_cancel_contract(status, &body);
    }
    assert!(app.events.events().is_empty());
}
