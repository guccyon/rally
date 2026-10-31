//! `/bookings` の統合テスト。student-app の `POST /bookings`（`createBooking`）と
//! `POST /bookings/{booking_id}/confirm`（`confirmBooking`）・`POST /bookings/{booking_id}/cancel`（`cancelBooking`）の
//! レスポンスの形を OpenAPI のスキーマで検査し、二重予約の拒否・体験予約の 1 回限り・確定でスロットが埋まること・
//! キャンセルでスロットが空くこととキャンセル規定の判定（`consumes_quota`）・発火するイベントを確かめる。
//! `GET /bookings/me`（`listMyBookings`）は本人の予約だけをスロットの開始時刻の新しい順に返すことと、生徒以外は 403 を確かめる。
//! `GET /coaches/me/bookings`（admin-web の `listMyCoachBookings`）はコーチ本人の枠への予約だけをスロットの開始時刻の古い順に、
//! 生徒の表示名を添えて返すことと、コーチ以外は 403 を確かめる。
//! `GET /coaches/me/slots`（admin-web の `listMyCoachSlots`）はコーチ本人の枠だけを、終わった枠・閉じた枠も含めて開始時刻の古い順に、
//! 有効な予約が入っているかと、結びついたレッスンメニュー（なければ null）を添えて返すことと、コーチ以外は 403 を確かめる。
//! 予約の一覧（`listMyBookings`・`listMyCoachBookings`）の枠には `lesson_menu` を載せない。予約には予約した時点のレッスンメニュー
//! `lesson_menu`（なければ null）を載せ、予約した後にメニューを直しても変わらないことを確かめる。

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
        FixedAiFeedbackGenerator, FixedClock, FixedVideoCallProvider, FixedVideoStorage,
        InMemoryAccountRepository, InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemoryLessonCompletionRepository, InMemoryLiveLessonSessionRepository,
        InMemorySlotRepository, InMemoryVideoRepository, RecordingAnalyticsSink,
        RecordingDomainEventPublisher,
    },
    lesson::RecordLessonCompletion,
    live_lesson::{EndLiveLessonSession, StartLiveLessonSession},
    scheduling::{
        CancelBooking, ConfirmBooking, CreateBooking, GetSlotSupply, ListCoachBookings,
        ListMyBookings, PublishSlot, SchedulingBookingLookup, SearchSlots, booking_id_uuid,
        slot_id_uuid,
    },
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    entitlement::{
        Entitlement, EntitlementId, LiveLessonQuota, SubscriptionId, SubscriptionPlan, YearMonth,
    },
    identity::{AccountId, Role},
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
        domain::scheduling::SlotSeats::default(),
    )
    .unwrap()
}

fn slot_id(id: u128) -> String {
    slot_id_uuid::format(SlotId::from_u128(id))
}

struct App {
    router: Router,
    slots: InMemorySlotRepository,
    /// 枠に結びついたレッスンメニューを引く保存先
    lesson_menus: application::in_memory::InMemoryLessonMenuRepository,
    accounts: InMemoryAccountRepository,
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
    let accounts = InMemoryAccountRepository::default();
    let lesson_menus = application::in_memory::InMemoryLessonMenuRepository::default();
    let router = interface::router_with(
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            analytics.clone(),
            clock,
        ),
        application::profile::ListCoaches::new(InMemoryAccountRepository::default()),
        application::identity::GetAccount::new(InMemoryAccountRepository::default()),
        application::profile::UpdateCoachProfile::new(InMemoryAccountRepository::default()),
        PublishSlot::new(
            slots.clone(),
            application::catalog::CatalogLessonMenuLookup::new(lesson_menus.clone()),
            analytics.clone(),
            clock,
        ),
        SearchSlots::new(
            slots.clone(),
            application::catalog::CatalogLessonMenuLookup::new(lesson_menus.clone()),
            application::profile::ProfileCoachIntroductionLookup::new(
                InMemoryAccountRepository::default(),
            ),
            clock,
        ),
        CreateBooking::new(
            bookings.clone(),
            application::catalog::CatalogLessonMenuLookup::new(lesson_menus.clone()),
            analytics.clone(),
            clock,
        ),
        ConfirmBooking::new(
            bookings.clone(),
            EntitlementQuotaGate::new(entitlements.clone()),
            analytics.clone(),
            events.clone(),
            clock,
        ),
        CancelBooking::new(bookings.clone(), analytics.clone(), events.clone(), clock),
        GetSlotSupply::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            clock,
        ),
        ListMyBookings::new(
            bookings.clone(),
            application::profile::ProfileCoachIntroductionLookup::new(accounts.clone()),
        ),
        ListCoachBookings::new(
            bookings.clone(),
            application::profile::ProfileStudentDisplayNameLookup::new(accounts.clone()),
        ),
        application::scheduling::ListCoachSlots::new(
            slots.clone(),
            application::catalog::CatalogLessonMenuLookup::new(lesson_menus.clone()),
            bookings,
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
            application::catalog::CatalogLessonMenuLookup::new(
                application::in_memory::InMemoryLessonMenuRepository::default(),
            ),
            FixedVideoStorage,
            FixedAiFeedbackGenerator,
            RecordingAnalyticsSink::default(),
            RecordingDomainEventPublisher::default(),
            clock,
        ),
        GetVideoReview::new(InMemoryVideoRepository::default()),
        ReturnVideoReview::new(
            InMemoryVideoRepository::default(),
            EntitlementReviewGate,
            RecordingAnalyticsSink::default(),
            events.clone(),
            clock,
        ),
        application::async_lesson::ListCoachVideos::new(
            InMemoryVideoRepository::default(),
            application::profile::ProfileStudentDisplayNameLookup::new(
                InMemoryAccountRepository::default(),
            ),
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
            events.clone(),
            clock,
        ),
        application::messaging::MarkMessageThreadRead::new(
            application::in_memory::InMemoryMessageThreadRepository::default(),
            clock,
        ),
        application::review::SubmitLessonReview::new(
            application::lesson::LessonCompletionLookup::new(
                application::in_memory::InMemoryLessonCompletionRepository::default(),
            ),
            application::in_memory::InMemoryLessonReviewRepository::default(),
            RecordingAnalyticsSink::default(),
            clock,
        ),
        application::review::GetCoachReviewSummary::new(
            application::in_memory::InMemoryLessonReviewRepository::default(),
        ),
        application::review::ListCoachReviews::new(
            application::in_memory::InMemoryLessonReviewRepository::default(),
            application::lesson::LessonCompletionMenuLookup::new(
                application::in_memory::InMemoryLessonCompletionRepository::default(),
                application::in_memory::InMemoryLiveLessonSessionRepository::default(),
                application::scheduling::SchedulingBookingLookup::new(
                    application::in_memory::InMemoryBookingRepository::new(
                        application::in_memory::InMemorySlotRepository::default(),
                    ),
                ),
                application::in_memory::InMemoryVideoRepository::default(),
            ),
        ),
        application::notification::GetNotificationSettings::new(
            application::in_memory::InMemoryNotificationSettingsRepository::default(),
        ),
        application::notification::UpdateNotificationSettings::new(
            application::in_memory::InMemoryNotificationSettingsRepository::default(),
        ),
        FakeTokens,
    );
    App {
        router,
        slots,
        lesson_menus,
        accounts,
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
            SubscriptionPlan::Standard,
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
        domain::scheduling::SlotSeats::default(),
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

// --- GET /bookings/me ---

async fn get_my_bookings(router: &Router, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get("/bookings/me");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    send(router, request.body(Body::empty()).unwrap()).await
}

fn assert_list_contract(status: StatusCode, body: &Value) {
    support::Contract::load("student-app").assert_response("get", "/bookings/me", status, body);
}

/// コーチ `COACH` のアカウントを保存する（契約にコーチの登録の操作がないので、保存先に直に入れる）。
async fn register_coach(app: &App) {
    use application::identity::AccountRepository;
    use domain::{
        identity::{Account, Email},
        profile::DisplayName,
    };

    let account = Account::new(
        COACH,
        Email::new("tanaka@example.com").unwrap(),
        DisplayName::new("たなか").unwrap(),
        Role::Coach,
        [Sport::Tennis],
        SystemTime::UNIX_EPOCH,
    )
    .unwrap();
    app.accounts.insert(account).await.unwrap();
}

#[tokio::test]
async fn student_lists_own_bookings_newest_slot_first_including_cancelled_ones() {
    let app = app();
    register_coach(&app).await;
    let first = create(&app.router, STUDENT, 10, true).await;
    let (status, body) = post_confirm(&app.router, Some(STUDENT), &first).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let cancelled = create(&app.router, STUDENT, 12, false).await;
    let (status, body) = post_cancel(&app.router, Some(STUDENT), &cancelled, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pending = create(&app.router, STUDENT, 11, false).await;
    // 別の生徒の予約は出ない
    let (status, body) = post_bookings(&app.router, Some(OTHER_STUDENT), booking(12, false)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let analytics = app.analytics.events().len();
    let events = app.events.events().len();

    let (status, body) = get_my_bookings(&app.router, Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_list_contract(status, &body);
    let items = body["bookings"].as_array().unwrap();
    let listed: Vec<(&str, &str, &str)> = items
        .iter()
        .map(|item| {
            (
                item["booking"]["booking_id"].as_str().unwrap(),
                item["booking"]["status"].as_str().unwrap(),
                item["slot"]["slot_id"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            (cancelled.as_str(), "cancelled", slot_id(12).as_str()),
            (pending.as_str(), "pending", slot_id(11).as_str()),
            (first.as_str(), "confirmed", slot_id(10).as_str()),
        ]
    );
    let student_id = account_id_uuid::format(account_id(STUDENT));
    assert!(
        items
            .iter()
            .all(|i| i["booking"]["student_id"] == student_id)
    );
    assert_eq!(
        items[2]["slot"],
        json!({
            "slot_id": slot_id(10),
            "coach_id": account_id_uuid::format(COACH),
            "sport": "tennis",
            "starts_at": "2026-11-01T10:00:00Z",
            "ends_at": "2026-11-01T11:00:00Z",
            "capacity": 1,
            "remaining_seats": 0,
            "coach_display_name": "たなか",
        }),
        "自己紹介文を書いていないコーチは coach_bio を載せない"
    );
    assert_eq!(items[2]["booking"]["is_trial"], true);
    // 照会では何も発火しない
    assert_eq!(app.analytics.events().len(), analytics);
    assert_eq!(app.events.events().len(), events);
}

#[tokio::test]
async fn student_without_bookings_gets_an_empty_list() {
    let app = app();
    create(&app.router, OTHER_STUDENT, 10, false).await;
    let (status, body) = get_my_bookings(&app.router, Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_list_contract(status, &body);
    assert_eq!(body, json!({ "bookings": [] }));
}

#[tokio::test]
async fn only_authenticated_students_can_list_their_bookings() {
    let app = app();
    create(&app.router, STUDENT, 10, false).await;
    for token in ["coach:9", "headquarters:5"] {
        let (status, body) = get_my_bookings(&app.router, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_list_contract(status, &body);
        assert_eq!(body["code"], "forbidden");
    }
    for token in [None, Some("unknown")] {
        let (status, body) = get_my_bookings(&app.router, token).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}: {body}");
        assert_list_contract(status, &body);
    }
}

// --- GET /coaches/me/bookings ---

const COACH_TOKEN: &str = "coach:9";

async fn get_my_coach_bookings(router: &Router, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get("/coaches/me/bookings");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    send(router, request.body(Body::empty()).unwrap()).await
}

fn assert_coach_list_contract(status: StatusCode, body: &Value) {
    support::Contract::load("admin-web").assert_response(
        "get",
        "/coaches/me/bookings",
        status,
        body,
    );
}

/// 生徒 `STUDENT` のアカウントを表示名「あおき」で保存する（`OTHER_STUDENT` は保存しない）。
async fn register_student(app: &App) {
    use application::identity::AccountRepository;
    use domain::{
        identity::{Account, Email},
        profile::DisplayName,
    };

    let account = Account::new(
        account_id(STUDENT),
        Email::new("aoki@example.com").unwrap(),
        DisplayName::new("あおき").unwrap(),
        Role::Student,
        [Sport::Tennis],
        SystemTime::UNIX_EPOCH,
    )
    .unwrap();
    app.accounts.insert(account).await.unwrap();
}

#[tokio::test]
async fn coach_lists_bookings_of_own_slots_oldest_slot_first_including_cancelled_ones() {
    // スロット 20 は別のコーチの枠
    let other_coach_slot = Slot::restore(
        SlotId::from_u128(20),
        AccountId::from_u128(8),
        Sport::Tennis,
        nov1(20),
        nov1(21),
        SlotStatus::Open,
        domain::scheduling::SlotSeats::default(),
    )
    .unwrap();
    let app = app_with([other_coach_slot]);
    register_student(&app).await;
    let cancelled = create(&app.router, STUDENT, 12, false).await;
    let (status, body) = post_cancel(&app.router, Some(STUDENT), &cancelled, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pending = create(&app.router, OTHER_STUDENT, 11, false).await;
    let confirmed = create(&app.router, STUDENT, 10, true).await;
    let (status, body) = post_confirm(&app.router, Some(STUDENT), &confirmed).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // 別のコーチの枠への予約は出ない
    create(&app.router, STUDENT, 20, false).await;
    let analytics = app.analytics.events().len();
    let events = app.events.events().len();

    let (status, body) = get_my_coach_bookings(&app.router, Some(COACH_TOKEN)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_coach_list_contract(status, &body);
    let items = body["bookings"].as_array().unwrap();
    let listed: Vec<(&str, &str, &str)> = items
        .iter()
        .map(|item| {
            (
                item["booking"]["booking_id"].as_str().unwrap(),
                item["booking"]["status"].as_str().unwrap(),
                item["slot"]["slot_id"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            (confirmed.as_str(), "confirmed", slot_id(10).as_str()),
            (pending.as_str(), "pending", slot_id(11).as_str()),
            (cancelled.as_str(), "cancelled", slot_id(12).as_str()),
        ]
    );
    let coach_id = account_id_uuid::format(COACH);
    assert!(
        items
            .iter()
            .all(|i| i["booking"]["coach_id"] == coach_id && i["slot"]["coach_id"] == coach_id)
    );
    assert_eq!(
        items[0]["slot"],
        json!({
            "slot_id": slot_id(10),
            "coach_id": coach_id,
            "sport": "tennis",
            "starts_at": "2026-11-01T10:00:00Z",
            "ends_at": "2026-11-01T11:00:00Z",
            "status": "booked",
            "capacity": 1,
            "remaining_seats": 0,
        })
    );
    assert_eq!(
        items[0]["booking"]["student_id"],
        account_id_uuid::format(account_id(STUDENT))
    );
    assert_eq!(items[0]["booking"]["is_trial"], true);
    assert_eq!(items[0]["student_display_name"], "あおき");
    assert_eq!(items[2]["student_display_name"], "あおき");
    assert!(
        items[1].get("student_display_name").is_none(),
        "アカウントが引けない生徒は student_display_name を載せない: {}",
        items[1]
    );
    // 照会では何も発火しない
    assert_eq!(app.analytics.events().len(), analytics);
    assert_eq!(app.events.events().len(), events);
}

#[tokio::test]
async fn coach_without_bookings_gets_an_empty_list() {
    let app = app();
    create(&app.router, STUDENT, 10, false).await;
    let (status, body) = get_my_coach_bookings(&app.router, Some("coach:8")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_coach_list_contract(status, &body);
    assert_eq!(body, json!({ "bookings": [] }));
}

#[tokio::test]
async fn only_authenticated_coaches_can_list_their_bookings() {
    let app = app();
    create(&app.router, STUDENT, 10, false).await;
    for token in [STUDENT, "headquarters:5"] {
        let (status, body) = get_my_coach_bookings(&app.router, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_coach_list_contract(status, &body);
        assert_eq!(body["code"], "forbidden");
    }
    for token in [None, Some("unknown")] {
        let (status, body) = get_my_coach_bookings(&app.router, token).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}: {body}");
        assert_coach_list_contract(status, &body);
    }
}

// --- GET /coaches/me/slots ---

async fn get_my_coach_slots(router: &Router, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get("/coaches/me/slots");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    send(router, request.body(Body::empty()).unwrap()).await
}

fn assert_coach_slot_list_contract(status: StatusCode, body: &Value) {
    support::Contract::load("admin-web").assert_response("get", "/coaches/me/slots", status, body);
}

#[tokio::test]
async fn coach_lists_own_slots_oldest_first_including_ended_ones_with_whether_booked() {
    // スロット 5 は終わった枠（現在時刻の 2 日前）、20 は別のコーチの枠
    let ended = Slot::restore(
        SlotId::from_u128(5),
        COACH,
        Sport::Tennis,
        SystemTime::UNIX_EPOCH + NOW - Duration::from_secs(48 * 3600),
        SystemTime::UNIX_EPOCH + NOW - Duration::from_secs(47 * 3600),
        SlotStatus::Open,
        domain::scheduling::SlotSeats::default(),
    )
    .unwrap();
    let other_coach_slot = Slot::restore(
        SlotId::from_u128(20),
        AccountId::from_u128(8),
        Sport::Tennis,
        nov1(20),
        nov1(21),
        SlotStatus::Open,
        domain::scheduling::SlotSeats::default(),
    )
    .unwrap();
    let app = app_with([ended, other_coach_slot]);
    // 10 は確定、11 は未確定、12 はキャンセル済みの予約だけ、13（閉じた枠）は予約なし
    let confirmed = create(&app.router, STUDENT, 10, true).await;
    let (status, body) = post_confirm(&app.router, Some(STUDENT), &confirmed).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    create(&app.router, OTHER_STUDENT, 11, false).await;
    let cancelled = create(&app.router, STUDENT, 12, false).await;
    let (status, body) = post_cancel(&app.router, Some(STUDENT), &cancelled, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    create(&app.router, STUDENT, 20, false).await;
    let analytics = app.analytics.events().len();
    let events = app.events.events().len();

    let (status, body) = get_my_coach_slots(&app.router, Some(COACH_TOKEN)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_coach_slot_list_contract(status, &body);
    let items = body["slots"].as_array().unwrap();
    let listed: Vec<(&str, &str, bool)> = items
        .iter()
        .map(|item| {
            (
                item["slot"]["slot_id"].as_str().unwrap(),
                item["slot"]["status"].as_str().unwrap(),
                item["has_active_booking"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            (slot_id(5).as_str(), "open", false),
            (slot_id(10).as_str(), "booked", true),
            (slot_id(11).as_str(), "open", true),
            (slot_id(12).as_str(), "open", false),
            (slot_id(13).as_str(), "closed", false),
        ],
        "開始時刻の古い順で、別のコーチの枠は出ない"
    );
    assert_eq!(
        items[1],
        json!({
            "slot": {
                "slot_id": slot_id(10),
                "coach_id": account_id_uuid::format(COACH),
                "sport": "tennis",
                "starts_at": "2026-11-01T10:00:00Z",
                "ends_at": "2026-11-01T11:00:00Z",
                "status": "booked",
                "capacity": 1,
                "remaining_seats": 0,
                "lesson_menu": null,
            },
            "has_active_booking": true,
        })
    );
    // 照会では何も発火しない
    assert_eq!(app.analytics.events().len(), analytics);
    assert_eq!(app.events.events().len(), events);
}

#[tokio::test]
async fn coach_slots_carry_the_lesson_menu_bound_to_each_slot() {
    use application::catalog::{LessonMenuRepository, lesson_menu_id_uuid};
    use domain::catalog::{
        LessonDuration, LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription,
        LessonMenuId, LessonMenuName,
    };

    // スロット 30 は公開中でなくなったメニュー 50 に、31 は catalog にないメニュー 51 に結びついている
    let bound = |id, menu| {
        slot(id, SlotStatus::Open).with_restored_lesson_menu(Some(LessonMenuId::from_u128(menu)))
    };
    let app = app_with([bound(30, 50), bound(31, 51)]);
    app.lesson_menus
        .insert(LessonMenu::restore(
            LessonMenuId::from_u128(50),
            COACH,
            LessonMenuContent {
                sport: Sport::Tennis,
                format: LessonFormat::Live(LessonDuration::from_minutes(60).unwrap()),
                name: LessonMenuName::new("フォア改善").unwrap(),
                description: LessonMenuDescription::default(),
            },
            false,
            nov1(0),
        ))
        .await
        .unwrap();

    let (status, body) = get_my_coach_slots(&app.router, Some(COACH_TOKEN)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_coach_slot_list_contract(status, &body);
    let menus: Vec<(&str, &Value)> = body["slots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            (
                item["slot"]["slot_id"].as_str().unwrap(),
                &item["slot"]["lesson_menu"],
            )
        })
        .collect();
    let menu = json!({
        "lesson_menu_id": lesson_menu_id_uuid::format(LessonMenuId::from_u128(50)),
        "name": "フォア改善",
        "duration_minutes": 60,
    });
    assert_eq!(
        menus,
        [
            (slot_id(10).as_str(), &Value::Null),
            (slot_id(11).as_str(), &Value::Null),
            (slot_id(12).as_str(), &Value::Null),
            (slot_id(13).as_str(), &Value::Null),
            (slot_id(30).as_str(), &menu),
            (slot_id(31).as_str(), &Value::Null),
        ],
        "コーチには非公開のメニューも載せ、メニューのない枠と catalog にないメニューは null"
    );

    // 予約の一覧の枠には載せない
    create(&app.router, STUDENT, 30, false).await;
    let (status, body) = get_my_coach_bookings(&app.router, Some(COACH_TOKEN)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["bookings"][0]["slot"].get("lesson_menu").is_none(),
        "{body}"
    );
}

#[tokio::test]
async fn bookings_carry_the_lesson_menu_of_the_time_of_booking() {
    use application::catalog::{LessonMenuRepository, lesson_menu_id_uuid};
    use domain::catalog::{
        LessonDuration, LessonFormat, LessonMenu, LessonMenuContent, LessonMenuId, LessonMenuName,
    };

    let menu = |id, name: &str, is_published| {
        LessonMenu::restore(
            LessonMenuId::from_u128(id),
            COACH,
            LessonMenuContent {
                sport: Sport::Tennis,
                format: LessonFormat::Live(LessonDuration::from_minutes(60).unwrap()),
                name: LessonMenuName::new(name).unwrap(),
                description: domain::catalog::LessonMenuDescription::default(),
            },
            is_published,
            nov1(0),
        )
    };
    // スロット 30 は公開中のメニュー 50 に、31 は非公開のメニュー 51 に結びついている
    let bound = |id, menu| {
        slot(id, SlotStatus::Open).with_restored_lesson_menu(Some(LessonMenuId::from_u128(menu)))
    };
    let app = app_with([bound(30, 50), bound(31, 51)]);
    register_coach(&app).await;
    register_student(&app).await;
    app.lesson_menus
        .insert(menu(50, "フォア改善", true))
        .await
        .unwrap();
    app.lesson_menus
        .insert(menu(51, "非公開", false))
        .await
        .unwrap();
    let copy = json!({
        "lesson_menu_id": lesson_menu_id_uuid::format(LessonMenuId::from_u128(50)),
        "name": "フォア改善",
        "duration_minutes": 60,
    });

    // 作成の応答に予約した時点のメニューを載せ、公開中でないメニューとメニューのない枠は null（予約は断らない）
    let (status, created) = post_bookings(&app.router, Some(STUDENT), booking(30, false)).await;
    assert_create_contract(status, &created);
    assert_eq!(created["lesson_menu"], copy);
    for slot in [31, 10] {
        let (status, body) = post_bookings(&app.router, Some(STUDENT), booking(slot, false)).await;
        assert_create_contract(status, &body);
        assert_eq!(body["lesson_menu"], Value::Null, "スロット {slot}");
    }
    let (status, confirmed) = post_confirm(
        &app.router,
        Some(STUDENT),
        created["booking_id"].as_str().unwrap(),
    )
    .await;
    assert_confirm_contract(status, &confirmed);
    assert_eq!(confirmed["lesson_menu"], copy);

    // 予約した後にメニューを直しても、予約の一覧は予約した時点のまま
    app.lesson_menus
        .update(menu(50, "直した名前", true))
        .await
        .unwrap();

    let (status, mine) = get_my_bookings(&app.router, Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{mine}");
    assert_list_contract(status, &mine);
    let (status, of_coach) = get_my_coach_bookings(&app.router, Some(COACH_TOKEN)).await;
    assert_eq!(status, StatusCode::OK, "{of_coach}");
    assert_coach_list_contract(status, &of_coach);
    for body in [&mine, &of_coach] {
        let mut menus: Vec<(&str, &Value)> = body["bookings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| {
                assert!(item["slot"].get("lesson_menu").is_none(), "{item}");
                (
                    item["slot"]["slot_id"].as_str().unwrap(),
                    &item["booking"]["lesson_menu"],
                )
            })
            .collect();
        menus.sort_by_key(|(slot, _)| *slot);
        assert_eq!(
            menus,
            [
                (slot_id(10).as_str(), &Value::Null),
                (slot_id(30).as_str(), &copy),
                (slot_id(31).as_str(), &Value::Null),
            ]
        );
    }
}

#[tokio::test]
async fn coach_without_slots_gets_an_empty_slot_list() {
    let app = app();
    let (status, body) = get_my_coach_slots(&app.router, Some("coach:8")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_coach_slot_list_contract(status, &body);
    assert_eq!(body, json!({ "slots": [] }));
}

#[tokio::test]
async fn only_authenticated_coaches_can_list_their_slots() {
    let app = app();
    for token in [STUDENT, "headquarters:5"] {
        let (status, body) = get_my_coach_slots(&app.router, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_coach_slot_list_contract(status, &body);
        assert_eq!(body["code"], "forbidden");
    }
    for token in [None, Some("unknown")] {
        let (status, body) = get_my_coach_slots(&app.router, token).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}: {body}");
        assert_coach_slot_list_contract(status, &body);
    }
}
