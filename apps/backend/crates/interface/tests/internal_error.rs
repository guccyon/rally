//! 500 応答に内部の理由が出ないことの検査。保存先が障害を返す `POST /accounts` で確かめる。

use std::time::SystemTime;

use application::{
    Unavailable,
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo},
    entitlement::{EntitlementQuotaGate, EntitlementReviewGate, GetEntitlement, StartSubscription},
    identity::{
        AccessToken, AccountRepository, InsertAccountError, InvalidToken, RegisterAccount,
        TokenClaims, TokenIssuer, TokenVerifier,
    },
    in_memory::{
        FixedAiFeedbackGenerator, FixedClock, FixedVideoCallProvider, FixedVideoStorage,
        InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemoryLessonCompletionRepository, InMemoryLiveLessonSessionRepository,
        InMemorySlotRepository, InMemoryVideoRepository, RecordingAnalyticsSink,
        RecordingDomainEventPublisher,
    },
    lesson::RecordLessonCompletion,
    live_lesson::{EndLiveLessonSession, StartLiveLessonSession},
    scheduling::{
        CancelBooking, ConfirmBooking, CreateBooking, GetSlotSupply, PublishSlot,
        SchedulingBookingLookup, SearchSlots,
    },
};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{Account, AccountId};
use serde_json::{Value, json};
use tower::ServiceExt;

/// 保存先の内部の理由（接続先・資格情報を含む想定）。
const REASON: &str = "connection to postgres://rally:secret@db.internal:5432 refused";

struct BrokenRepository;

impl AccountRepository for BrokenRepository {
    fn next_id(&self) -> AccountId {
        AccountId::from_u128(1)
    }

    async fn insert(&self, _account: Account) -> Result<(), InsertAccountError> {
        Err(InsertAccountError::Unavailable(Unavailable::new(REASON)))
    }

    async fn find(&self, _id: AccountId) -> Result<Option<Account>, Unavailable> {
        Err(Unavailable::new(REASON))
    }

    async fn list_coaches(&self) -> Result<Vec<Account>, Unavailable> {
        Err(Unavailable::new(REASON))
    }

    async fn update_coach_bio(
        &self,
        _coach_id: AccountId,
        _bio: domain::profile::CoachBio,
    ) -> Result<Option<Account>, Unavailable> {
        Err(Unavailable::new(REASON))
    }

    async fn update_student_display_name(
        &self,
        _student_id: AccountId,
        _display_name: domain::profile::DisplayName,
    ) -> Result<Option<Account>, Unavailable> {
        Err(Unavailable::new(REASON))
    }
}

struct NoTokens;

impl TokenIssuer for NoTokens {
    fn issue(&self, _claims: TokenClaims) -> AccessToken {
        unreachable!("保存に失敗したら発行しない")
    }
}

impl TokenVerifier for NoTokens {
    fn verify(&self, _token: &str) -> Result<TokenClaims, InvalidToken> {
        Err(InvalidToken)
    }
}

#[tokio::test]
async fn storage_failure_is_500_internal_without_the_reason() {
    let analytics = RecordingAnalyticsSink::default();
    let clock = FixedClock(SystemTime::UNIX_EPOCH);
    let slots = InMemorySlotRepository::default();
    let bookings = InMemoryBookingRepository::new(slots.clone());
    let router = interface::router_with(
        RegisterAccount::new(BrokenRepository, analytics.clone(), clock),
        application::profile::ListCoaches::new(BrokenRepository),
        application::identity::GetAccount::new(BrokenRepository),
        application::profile::UpdateCoachProfile::new(BrokenRepository),
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
            application::profile::ProfileCoachIntroductionLookup::new(BrokenRepository),
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
            bookings,
            analytics.clone(),
            RecordingDomainEventPublisher::default(),
            clock,
        ),
        GetSlotSupply::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            clock,
        ),
        application::scheduling::ListMyBookings::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            application::profile::ProfileCoachIntroductionLookup::new(BrokenRepository),
        ),
        application::scheduling::ListCoachBookings::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            application::profile::ProfileStudentDisplayNameLookup::new(BrokenRepository),
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
            application::in_memory::RecordingDomainEventPublisher::default(),
            clock,
        ),
        GetVideoReview::new(InMemoryVideoRepository::default()),
        ReturnVideoReview::new(
            InMemoryVideoRepository::default(),
            EntitlementReviewGate,
            RecordingAnalyticsSink::default(),
            application::in_memory::RecordingDomainEventPublisher::default(),
            clock,
        ),
        application::async_lesson::ListCoachVideos::new(
            InMemoryVideoRepository::default(),
            application::profile::ProfileStudentDisplayNameLookup::new(BrokenRepository),
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
        NoTokens,
    );

    let response = router
        .oneshot(
            Request::post("/accounts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "tennis" })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    for fragment in [REASON, "postgres", "secret", "refused"] {
        assert!(!text.contains(fragment), "内部の理由が出ている: {text}");
    }
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], "internal");
    assert!(body["message"].is_string());
    assert!(analytics.events().is_empty());
}
