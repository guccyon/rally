//! `GET /slot-supply`（admin-web の `getSlotSupply`）の統合テスト。レスポンスの形を admin-web の OpenAPI の
//! スキーマで検査し、本部だけが呼べることと、期間に開始する枠だけを数えることと、日付（`from`・`to`）で期間を指定できる
//! ことを確かめる。判定の式の境界は
//! domain の `SlotSupply` のテスト。

use std::time::{Duration, SystemTime};

use application::{
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo},
    entitlement::{EntitlementQuotaGate, EntitlementReviewGate, GetEntitlement, StartSubscription},
    identity::{
        AccessToken, InvalidToken, RegisterAccount, TokenClaims, TokenIssuer, TokenVerifier,
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
        BookingRepository, CancelBooking, ConfirmBooking, CreateBooking, GetSlotSupply,
        PublishSlot, SchedulingBookingLookup, SearchSlots,
    },
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    identity::{AccountId, Role},
    scheduling::{Booking, Slot, SlotId, SlotStatus},
};
use serde_json::{Value, json};
use tower::ServiceExt;

mod support;

/// 2026-10-26T00:00:00Z（現在時刻）
const NOW: Duration = Duration::from_secs(1_792_972_800);
const HOUR: Duration = Duration::from_secs(3600);
const DAY: Duration = Duration::from_secs(24 * 3600);

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

fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + NOW
}

fn slot(id: u128, sport: Sport, starts_at: SystemTime, status: SlotStatus) -> Slot {
    Slot::restore(
        SlotId::from_u128(id),
        AccountId::from_u128(100 + id),
        sport,
        starts_at,
        starts_at + HOUR,
        status,
        domain::scheduling::SlotSeats::default(),
    )
    .unwrap()
}

async fn book(bookings: &InMemoryBookingRepository, student: u128, slot: u128) {
    let student = AccountId::from_u128(student);
    bookings
        .reserve(SlotId::from_u128(slot), student, |slot, existing| {
            Booking::reserve(bookings.next_id(), student, false, slot, existing, now())
        })
        .await
        .unwrap();
}

/// 期間（30 日前から今まで）に開始するテニスの枠 3 つ（1 つは閉じた枠）に 2 人、サッカーの枠 2 つに 1 人が予約した。
/// 期間の外（30 日と 1 時間前・今から 1 日後）のテニスの枠にも予約がある。
async fn bookings() -> InMemoryBookingRepository {
    let slots = InMemorySlotRepository::with_slots([
        slot(1, Sport::Tennis, now() - 30 * DAY, SlotStatus::Open),
        slot(2, Sport::Tennis, now() - DAY, SlotStatus::Open),
        slot(3, Sport::Tennis, now() - 2 * DAY, SlotStatus::Open),
        slot(4, Sport::Tennis, now() - 3 * DAY, SlotStatus::Closed),
        slot(5, Sport::Soccer, now() - DAY, SlotStatus::Open),
        slot(6, Sport::Soccer, now() - 2 * DAY, SlotStatus::Open),
        slot(7, Sport::Tennis, now() - 30 * DAY - HOUR, SlotStatus::Open),
        slot(8, Sport::Tennis, now() + DAY, SlotStatus::Open),
    ]);
    let bookings = InMemoryBookingRepository::new(slots);
    book(&bookings, 11, 1).await;
    book(&bookings, 11, 2).await;
    book(&bookings, 12, 3).await;
    book(&bookings, 13, 5).await;
    book(&bookings, 14, 7).await;
    book(&bookings, 15, 8).await;
    bookings
}

fn app(bookings: InMemoryBookingRepository) -> Router {
    let analytics = RecordingAnalyticsSink::default();
    let clock = FixedClock(now());
    let slots = bookings.slots().clone();
    interface::router_with(
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            analytics.clone(),
            clock,
        ),
        application::identity::GetAccount::new(InMemoryAccountRepository::default()),
        application::profile::GetCoachBio::new(InMemoryAccountRepository::default()),
        application::profile::UpdateCoachProfile::new(InMemoryAccountRepository::default()),
        PublishSlot::new(
            slots.clone(),
            application::catalog::CatalogLessonMenuLookup::new(
                application::in_memory::InMemoryLessonMenuRepository::default(),
            ),
            analytics.clone(),
            clock,
        ),
        SearchSlots::new(
            slots,
            application::catalog::CatalogLessonMenuLookup::new(
                application::in_memory::InMemoryLessonMenuRepository::default(),
            ),
            application::profile::ProfileCoachIntroductionLookup::new(
                InMemoryAccountRepository::default(),
            ),
            clock,
        ),
        CreateBooking::new(
            bookings.clone(),
            application::catalog::CatalogLessonMenuLookup::new(
                application::in_memory::InMemoryLessonMenuRepository::default(),
            ),
            analytics.clone(),
            clock,
        ),
        ConfirmBooking::new(
            bookings.clone(),
            EntitlementQuotaGate::new(InMemoryEntitlementRepository::default()),
            analytics.clone(),
            RecordingDomainEventPublisher::default(),
            clock,
        ),
        CancelBooking::new(
            bookings.clone(),
            analytics.clone(),
            RecordingDomainEventPublisher::default(),
            clock,
        ),
        GetSlotSupply::new(bookings, clock),
        application::scheduling::ListMyBookings::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            application::profile::ProfileCoachIntroductionLookup::new(
                InMemoryAccountRepository::default(),
            ),
            EntitlementQuotaGate::new(InMemoryEntitlementRepository::default()),
            clock,
        ),
        application::scheduling::ListCoachBookings::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            application::profile::ProfileStudentDisplayNameLookup::new(
                InMemoryAccountRepository::default(),
            ),
            clock,
        ),
        application::scheduling::ListCoachSlots::new(
            InMemorySlotRepository::default(),
            application::catalog::CatalogLessonMenuLookup::new(
                application::in_memory::InMemoryLessonMenuRepository::default(),
            ),
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
        ),
        StartSubscription::new(
            InMemoryEntitlementRepository::default(),
            analytics.clone(),
            RecordingDomainEventPublisher::default(),
            clock,
        ),
        GetEntitlement::new(InMemoryEntitlementRepository::default(), clock),
        StartLiveLessonSession::new(
            InMemoryLiveLessonSessionRepository::default(),
            SchedulingBookingLookup::new(InMemoryBookingRepository::new(
                InMemorySlotRepository::default(),
            )),
            FixedVideoCallProvider::default(),
            analytics.clone(),
            clock,
        ),
        EndLiveLessonSession::new(
            InMemoryLiveLessonSessionRepository::default(),
            FixedVideoCallProvider::default(),
            analytics.clone(),
            clock,
        ),
        SubmitVideo::new(
            InMemoryVideoRepository::default(),
            application::catalog::CatalogLessonMenuLookup::new(
                application::in_memory::InMemoryLessonMenuRepository::default(),
            ),
            FixedVideoStorage,
            FixedAiFeedbackGenerator,
            analytics.clone(),
            application::in_memory::RecordingDomainEventPublisher::default(),
            clock,
        ),
        GetVideoReview::new(InMemoryVideoRepository::default()),
        ReturnVideoReview::new(
            InMemoryVideoRepository::default(),
            EntitlementReviewGate,
            analytics.clone(),
            application::in_memory::RecordingDomainEventPublisher::default(),
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
            analytics,
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
            application::in_memory::RecordingDomainEventPublisher::default(),
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
    )
}

async fn get_slot_supply(router: &Router, token: Option<&str>) -> (StatusCode, Value) {
    get_slot_supply_at(router, "/slot-supply", token).await
}

/// `uri` はクエリ（`from`・`to`）を付けた `/slot-supply`。
async fn get_slot_supply_at(
    router: &Router,
    uri: &str,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = Request::get(uri);
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
    support::Contract::load("admin-web").assert_response("get", "/slot-supply", status, &body);
    (status, body)
}

#[tokio::test]
async fn headquarters_reads_the_supply_of_every_sport_in_the_last_thirty_days() {
    let router = app(bookings().await);
    let (status, body) = get_slot_supply(&router, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "window_start": "2026-09-26T00:00:00Z",
            "window_end": "2026-10-26T00:00:00Z",
            "sports": [
                // 開いている 3 枠に 2 人（67%）。閉じた枠は数えない
                { "sport": "tennis", "status": "insufficient", "published_slots": 3, "booking_students": 2 },
                { "sport": "golf", "status": "sufficient", "published_slots": 0, "booking_students": 0 },
                { "sport": "baseball", "status": "sufficient", "published_slots": 0, "booking_students": 0 },
                // 2 枠に 1 人（50%）
                { "sport": "soccer", "status": "sufficient", "published_slots": 2, "booking_students": 1 },
                { "sport": "basketball", "status": "sufficient", "published_slots": 0, "booking_students": 0 },
                { "sport": "swimming", "status": "sufficient", "published_slots": 0, "booking_students": 0 },
            ],
        })
    );
}

#[tokio::test]
async fn only_headquarters_may_read_the_supply() {
    let router = app(bookings().await);
    for token in ["coach:2", "student:3"] {
        let (status, body) = get_slot_supply(&router, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
    }
    let (status, body) = get_slot_supply(&router, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "unauthorized");
}

#[tokio::test]
async fn headquarters_reads_the_supply_of_the_dates_in_japan() {
    let router = app(bookings().await);
    // 明日（日本時間の 10 月 27 日）だけ。テニスの枠 1 つ（10 月 27 日 9 時）に 1 人
    let (status, body) = get_slot_supply_at(
        &router,
        "/slot-supply?from=2026-10-27&to=2026-10-27",
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["window_start"], "2026-10-26T15:00:00Z");
    assert_eq!(body["window_end"], "2026-10-27T15:00:00Z");
    assert_eq!(
        (&body["from"], &body["to"]),
        (&json!("2026-10-27"), &json!("2026-10-27"))
    );
    assert_eq!(
        body["sports"][0],
        json!({ "sport": "tennis", "status": "insufficient", "published_slots": 1, "booking_students": 1 })
    );
    assert_eq!(body["sports"][3]["published_slots"], 0, "{body}");

    // 両方省けば今までどおり（`from`・`to` は載せない）
    let (_, body) = get_slot_supply(&router, Some(HEADQUARTERS)).await;
    assert!(
        body.get("from").is_none() && body.get("to").is_none(),
        "{body}"
    );
}

#[tokio::test]
async fn a_missing_date_is_decided_like_list_bookings() {
    let router = app(bookings().await);
    // from だけなら from から 7 日目まで
    let (status, body) =
        get_slot_supply_at(&router, "/slot-supply?from=2026-10-27", Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (&body["from"], &body["to"]),
        (&json!("2026-10-27"), &json!("2026-11-02"))
    );
    assert_eq!(body["window_end"], "2026-11-02T15:00:00Z");
    // to だけなら今日（日本時間の 10 月 26 日）から。今日の 9 時より前に始まった枠も今日なら入る
    let (status, body) =
        get_slot_supply_at(&router, "/slot-supply?to=2026-10-27", Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (&body["from"], &body["to"]),
        (&json!("2026-10-26"), &json!("2026-10-27"))
    );
    assert_eq!(body["window_start"], "2026-10-25T15:00:00Z");
}

#[tokio::test]
async fn dates_breaking_the_rule_are_unprocessable() {
    let router = app(bookings().await);
    for (query, message) in [
        (
            "from=2026-10-28&to=2026-10-27",
            "to は from と同じ日か、それより後の日付にする",
        ),
        (
            "from=2026-10-01&to=2026-11-01",
            "from から to までは 31 日まで",
        ),
        // to だけで今日より前は from（今日）より前
        (
            "to=2026-10-25",
            "to は from と同じ日か、それより後の日付にする",
        ),
        ("from=2026/10/27", "from は YYYY-MM-DD の日付でない"),
        ("to=2026-02-30", "to は YYYY-MM-DD の日付でない"),
    ] {
        let (status, body) = get_slot_supply_at(
            &router,
            &format!("/slot-supply?{query}"),
            Some(HEADQUARTERS),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}: {body}");
        assert_eq!(body["code"], "validation_failed", "{query}");
        assert_eq!(body["message"], message, "{query}");
    }
    // 31 日（両端を含む）は通る
    let (status, body) = get_slot_supply_at(
        &router,
        "/slot-supply?from=2026-10-01&to=2026-10-31",
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}
