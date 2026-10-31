//! `GET /coaches`（admin-web の `listCoaches`）の統合テスト。レスポンスの形を admin-web の OpenAPI のスキーマで
//! 検査し、本部だけが呼べることと、コーチだけを表示名の順で返すことを確かめる。

use std::time::SystemTime;

use application::{
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo},
    entitlement::{EntitlementQuotaGate, EntitlementReviewGate, GetEntitlement, StartSubscription},
    identity::{
        AccessToken, AccountRepository, GetAccount, InvalidToken, RegisterAccount, TokenClaims,
        TokenIssuer, TokenVerifier,
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
    profile::ListCoaches,
    scheduling::{
        CancelBooking, ConfirmBooking, CreateBooking, GetSlotSupply, PublishSlot,
        SchedulingBookingLookup, SearchSlots,
    },
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

fn account(id: u128, display_name: &str, role: Role, sports: &[Sport]) -> Account {
    Account::new(
        AccountId::from_u128(id),
        Email::new(format!("user{id}@example.com")).unwrap(),
        DisplayName::new(display_name).unwrap(),
        role,
        sports.iter().copied(),
        SystemTime::UNIX_EPOCH,
    )
    .unwrap()
}

/// コーチ 3 人（うち 2 人は同じ表示名）と、生徒・本部が 1 人ずつ。
async fn accounts() -> InMemoryAccountRepository {
    let accounts = InMemoryAccountRepository::default();
    for account in [
        account(1, "本部", Role::Headquarters, &[]),
        account(2, "たなか", Role::Coach, &[Sport::Tennis]),
        account(3, "あおき", Role::Student, &[Sport::Tennis]),
        account(5, "いとう", Role::Coach, &[Sport::Soccer, Sport::Golf]),
        account(4, "いとう", Role::Coach, &[Sport::Baseball]),
    ] {
        accounts.insert(account).await.unwrap();
    }
    accounts
}

fn app(accounts: InMemoryAccountRepository) -> Router {
    let analytics = RecordingAnalyticsSink::default();
    let clock = FixedClock(SystemTime::UNIX_EPOCH);
    let slots = InMemorySlotRepository::default();
    let bookings = InMemoryBookingRepository::new(slots.clone());
    interface::router_with(
        RegisterAccount::new(accounts.clone(), analytics.clone(), clock),
        ListCoaches::new(accounts.clone()),
        GetAccount::new(accounts.clone()),
        application::profile::UpdateCoachProfile::new(accounts),
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
        ),
        application::scheduling::ListCoachBookings::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            application::profile::ProfileStudentDisplayNameLookup::new(
                InMemoryAccountRepository::default(),
            ),
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

async fn list_coaches(router: &Router, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get("/coaches");
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
    support::Contract::load("admin-web").assert_response("get", "/coaches", status, &body);
    (status, body)
}

#[tokio::test]
async fn headquarters_lists_only_coaches_in_display_name_order() {
    let router = app(accounts().await);
    let (status, body) = list_coaches(&router, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "coaches": [
                // 同じ表示名は ID の順
                { "coach_id": "00000000-0000-0000-0000-000000000004", "display_name": "いとう" },
                { "coach_id": "00000000-0000-0000-0000-000000000005", "display_name": "いとう" },
                { "coach_id": "00000000-0000-0000-0000-000000000002", "display_name": "たなか" },
            ],
        })
    );
}

#[tokio::test]
async fn no_coaches_is_an_empty_list() {
    let router = app(InMemoryAccountRepository::default());
    let (status, body) = list_coaches(&router, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "coaches": [] }));
}

#[tokio::test]
async fn only_headquarters_may_list_coaches() {
    let router = app(accounts().await);
    for token in ["coach:2", "student:3"] {
        let (status, body) = list_coaches(&router, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
    }
    let (status, body) = list_coaches(&router, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "unauthorized");
}
