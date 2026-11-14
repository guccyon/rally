//! `GET /slot-supply`（admin-web の `getSlotSupply`）の統合テスト。レスポンスの形を admin-web の OpenAPI の
//! スキーマで検査し、本部だけが呼べることと、期間に開始する枠だけを数えることと、日付（`from`・`to`）で期間を指定できる
//! ことと、`coach_id` でそのコーチの枠だけを数えることと、`sport` でその競技を提供するコーチの枠だけを数えることと、
//! `lesson_menu_id` でそのメニューに結びついた枠だけを数えることを確かめる。判定の式の境界は
//! domain の `SlotSupply` のテスト。

use std::time::{Duration, SystemTime};

use application::{
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo},
    entitlement::{EntitlementQuotaGate, EntitlementReviewGate, GetEntitlement, StartSubscription},
    identity::{
        AccessToken, AccountRepository, InvalidToken, RegisterAccount, TokenClaims, TokenIssuer,
        TokenVerifier,
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
    catalog::{LessonMenuId, Sport},
    identity::{Account, AccountId, Email, Role},
    profile::DisplayName,
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

/// [`bookings`] の枠のコーチ（ID は 100 + 枠の ID）のアカウント。提供スポーツは枠のスポーツで、枠 2（テニス）のコーチは
/// サッカーも提供する。
async fn coach_accounts() -> InMemoryAccountRepository {
    let accounts = InMemoryAccountRepository::default();
    for (id, sports) in [
        (101, &[Sport::Tennis][..]),
        (102, &[Sport::Tennis, Sport::Soccer]),
        (103, &[Sport::Tennis]),
        (104, &[Sport::Tennis]),
        (105, &[Sport::Soccer]),
        (106, &[Sport::Soccer]),
        (107, &[Sport::Tennis]),
        (108, &[Sport::Tennis]),
    ] {
        let account = Account::new(
            AccountId::from_u128(id),
            Email::new(format!("coach{id}@example.com")).unwrap(),
            DisplayName::new("コーチ").unwrap(),
            Role::Coach,
            sports.iter().copied(),
            now(),
        )
        .unwrap();
        accounts.insert(account).await.unwrap();
    }
    accounts
}

fn app(bookings: InMemoryBookingRepository) -> Router {
    app_with_accounts(bookings, InMemoryAccountRepository::default())
}

fn app_with_accounts(
    bookings: InMemoryBookingRepository,
    accounts: InMemoryAccountRepository,
) -> Router {
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
            application::in_memory::InMemoryLessonCompletionRepository::default(),
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
        GetSlotSupply::new(
            bookings,
            application::profile::ProfileCoachAccountLookup::new(accounts),
            clock,
        ),
        application::scheduling::ListMyBookings::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            application::profile::ProfileCoachIntroductionLookup::new(
                InMemoryAccountRepository::default(),
            ),
            EntitlementQuotaGate::new(InMemoryEntitlementRepository::default()),
            application::in_memory::InMemoryLessonCompletionRepository::default(),
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
            clock,
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
                { "sport": "golf", "status": "not_applicable", "published_slots": 0, "booking_students": 0 },
                { "sport": "baseball", "status": "no_activity", "published_slots": 0, "booking_students": 0 },
                // 2 枠に 1 人（50%）
                { "sport": "soccer", "status": "sufficient", "published_slots": 2, "booking_students": 1 },
                { "sport": "basketball", "status": "no_activity", "published_slots": 0, "booking_students": 0 },
                { "sport": "swimming", "status": "no_activity", "published_slots": 0, "booking_students": 0 },
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

fn account_uuid(id: u128) -> String {
    format!("00000000-0000-0000-0000-{id:012x}")
}

/// どのスポーツも枠も予約しようとした生徒も 0 か（判定はゴルフが対象外、それ以外が枠も予約もない）。
fn all_zero(body: &Value) -> bool {
    body["sports"].as_array().unwrap().iter().all(|supply| {
        let status = if supply["sport"] == "golf" {
            "not_applicable"
        } else {
            "no_activity"
        };
        supply["published_slots"] == 0
            && supply["booking_students"] == 0
            && supply["status"] == status
    })
}

#[tokio::test]
async fn coach_id_counts_only_the_slots_of_that_coach() {
    let router = app(bookings().await);
    // 枠 2（テニス、1 日前）のコーチ。その枠に 1 人
    let (status, body) = get_slot_supply_at(
        &router,
        &format!("/slot-supply?coach_id={}", account_uuid(102)),
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["window_start"], "2026-09-26T00:00:00Z");
    assert_eq!(body["window_end"], "2026-10-26T00:00:00Z");
    assert!(
        body.get("from").is_none() && body.get("to").is_none(),
        "{body}"
    );
    assert_eq!(
        body["sports"][0],
        json!({ "sport": "tennis", "status": "insufficient", "published_slots": 1, "booking_students": 1 })
    );
    assert_eq!(body["sports"][3]["published_slots"], 0, "{body}");
    assert_eq!(body["sports"].as_array().unwrap().len(), 6);

    // 期間と組み合わせる。明日の枠 8 のコーチなら 1 枠に 1 人、枠 2 のコーチは 0
    let (status, body) = get_slot_supply_at(
        &router,
        &format!(
            "/slot-supply?from=2026-10-27&to=2026-10-27&coach_id={}",
            account_uuid(108)
        ),
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["from"], "2026-10-27");
    assert_eq!(
        body["sports"][0],
        json!({ "sport": "tennis", "status": "insufficient", "published_slots": 1, "booking_students": 1 })
    );
    let (status, body) = get_slot_supply_at(
        &router,
        &format!(
            "/slot-supply?from=2026-10-27&to=2026-10-27&coach_id={}",
            account_uuid(102)
        ),
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(all_zero(&body), "{body}");
}

#[tokio::test]
async fn coach_id_of_no_coach_is_zero_and_not_a_uuid_is_unprocessable() {
    let router = app(bookings().await);
    // 予約した生徒（コーチでない ID）・ない ID は 404 にせず、どのスポーツも 0
    for coach_id in [account_uuid(11), account_uuid(1000)] {
        let (status, body) = get_slot_supply_at(
            &router,
            &format!("/slot-supply?coach_id={coach_id}"),
            Some(HEADQUARTERS),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{coach_id}: {body}");
        assert!(all_zero(&body), "{coach_id}: {body}");
    }
    for coach_id in ["not-a-uuid", ""] {
        let (status, body) = get_slot_supply_at(
            &router,
            &format!("/slot-supply?coach_id={coach_id}"),
            Some(HEADQUARTERS),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{coach_id}: {body}"
        );
        assert_eq!(body["code"], "validation_failed");
        assert_eq!(body["message"], "coach_id が UUID でない");
    }
}

#[tokio::test]
async fn sport_counts_only_the_slots_of_coaches_offering_it() {
    let router = app_with_accounts(bookings().await, coach_accounts().await);
    // サッカーを提供するのは枠 2（テニス）・5・6 のコーチ。枠 2 に 1 人、枠 5 に 1 人
    let (status, body) =
        get_slot_supply_at(&router, "/slot-supply?sport=soccer", Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["window_start"], "2026-09-26T00:00:00Z");
    assert_eq!(body["window_end"], "2026-10-26T00:00:00Z");
    assert_eq!(body["sports"].as_array().unwrap().len(), 6);
    assert_eq!(
        body["sports"][0],
        json!({ "sport": "tennis", "status": "insufficient", "published_slots": 1, "booking_students": 1 })
    );
    assert_eq!(
        body["sports"][3],
        json!({ "sport": "soccer", "status": "sufficient", "published_slots": 2, "booking_students": 1 })
    );

    // 提供するコーチのいない競技は、どのスポーツも 0
    let (status, body) =
        get_slot_supply_at(&router, "/slot-supply?sport=golf", Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(all_zero(&body), "{body}");

    // コーチ・期間と組み合わせる。枠 2 のコーチはサッカーを提供するので、その枠だけ
    let (status, body) = get_slot_supply_at(
        &router,
        &format!("/slot-supply?sport=soccer&coach_id={}", account_uuid(102)),
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["sports"][0],
        json!({ "sport": "tennis", "status": "insufficient", "published_slots": 1, "booking_students": 1 })
    );
    assert_eq!(body["sports"][3]["published_slots"], 0, "{body}");
    let (status, body) = get_slot_supply_at(
        &router,
        "/slot-supply?from=2026-10-27&to=2026-10-27&sport=tennis",
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["from"], "2026-10-27");
    assert_eq!(
        body["sports"][0],
        json!({ "sport": "tennis", "status": "insufficient", "published_slots": 1, "booking_students": 1 })
    );

    // 付けなければ今までどおり
    let (status, body) = get_slot_supply(&router, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["sports"][3]["published_slots"], 2, "{body}");
    assert_eq!(body["sports"][0]["published_slots"], 3, "{body}");
}

#[tokio::test]
async fn unknown_sport_is_unprocessable() {
    let router = app_with_accounts(bookings().await, coach_accounts().await);
    for sport in ["cricket", "Tennis", ""] {
        let (status, body) = get_slot_supply_at(
            &router,
            &format!("/slot-supply?sport={sport}"),
            Some(HEADQUARTERS),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{sport}: {body}");
        assert_eq!(body["code"], "validation_failed");
        assert_eq!(body["message"], format!("未知のスポーツ: {sport}"));
    }
}

fn menu_uuid(id: u128) -> String {
    format!("00000000-0000-0000-0000-{id:012x}")
}

/// 期間（30 日前から今まで）に、メニュー 50 に結びついたテニスの枠 2 つ（1 つに 1 人）と、メニュー 51 のサッカーの枠
/// 1 つ（1 人）と、メニューのないテニスの枠 1 つ（1 人）。明日のメニュー 50 の枠にも 1 人。
async fn menu_bookings() -> InMemoryBookingRepository {
    let menu = |id| Some(LessonMenuId::from_u128(id));
    let slots = InMemorySlotRepository::with_slots([
        slot(1, Sport::Tennis, now() - DAY, SlotStatus::Open).with_restored_lesson_menu(menu(50)),
        slot(2, Sport::Tennis, now() - 2 * DAY, SlotStatus::Open)
            .with_restored_lesson_menu(menu(50)),
        slot(3, Sport::Soccer, now() - DAY, SlotStatus::Open).with_restored_lesson_menu(menu(51)),
        slot(4, Sport::Tennis, now() - DAY, SlotStatus::Open),
        slot(5, Sport::Tennis, now() + DAY, SlotStatus::Open).with_restored_lesson_menu(menu(50)),
    ]);
    let bookings = InMemoryBookingRepository::new(slots);
    book(&bookings, 11, 1).await;
    book(&bookings, 12, 3).await;
    book(&bookings, 13, 4).await;
    book(&bookings, 14, 5).await;
    bookings
}

#[tokio::test]
async fn lesson_menu_id_counts_only_the_slots_bound_to_that_menu() {
    let router = app(menu_bookings().await);
    // メニュー 50 の 2 枠に 1 人。メニュー 51 の枠・メニューのない枠と、それらへの予約は数えない
    let (status, body) = get_slot_supply_at(
        &router,
        &format!("/slot-supply?lesson_menu_id={}", menu_uuid(50)),
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["window_start"], "2026-09-26T00:00:00Z");
    assert_eq!(body["window_end"], "2026-10-26T00:00:00Z");
    assert!(
        body.get("from").is_none() && body.get("to").is_none(),
        "{body}"
    );
    assert_eq!(body["sports"].as_array().unwrap().len(), 6);
    assert_eq!(
        body["sports"][0],
        json!({ "sport": "tennis", "status": "sufficient", "published_slots": 2, "booking_students": 1 })
    );
    assert_eq!(body["sports"][3]["published_slots"], 0, "{body}");

    // 期間・コーチと組み合わせる。明日はメニュー 50 の枠 5 だけ。枠 1 のコーチなら枠 1 だけ
    let (status, body) = get_slot_supply_at(
        &router,
        &format!(
            "/slot-supply?from=2026-10-27&to=2026-10-27&lesson_menu_id={}",
            menu_uuid(50)
        ),
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["from"], "2026-10-27");
    assert_eq!(
        body["sports"][0],
        json!({ "sport": "tennis", "status": "insufficient", "published_slots": 1, "booking_students": 1 })
    );
    let (status, body) = get_slot_supply_at(
        &router,
        &format!(
            "/slot-supply?lesson_menu_id={}&coach_id={}",
            menu_uuid(50),
            account_uuid(101)
        ),
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["sports"][0],
        json!({ "sport": "tennis", "status": "insufficient", "published_slots": 1, "booking_students": 1 })
    );

    // ないメニューの ID は 404 にせず、どのスポーツも 0
    let (status, body) = get_slot_supply_at(
        &router,
        &format!("/slot-supply?lesson_menu_id={}", menu_uuid(999)),
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(all_zero(&body), "{body}");

    // 付けなければ今までどおりメニューを問わない
    let (status, body) = get_slot_supply(&router, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["sports"][0]["published_slots"], 3, "{body}");
    assert_eq!(body["sports"][3]["published_slots"], 1, "{body}");
}

#[tokio::test]
async fn lesson_menu_id_not_a_uuid_is_unprocessable() {
    let router = app(menu_bookings().await);
    for lesson_menu_id in ["not-a-uuid", ""] {
        let (status, body) = get_slot_supply_at(
            &router,
            &format!("/slot-supply?lesson_menu_id={lesson_menu_id}"),
            Some(HEADQUARTERS),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{lesson_menu_id}: {body}"
        );
        assert_eq!(body["code"], "validation_failed");
        assert_eq!(body["message"], "lesson_menu_id が UUID でない");
    }
}
