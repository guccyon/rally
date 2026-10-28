//! `POST /subscriptions`（student-app の `startSubscription`）の統合テスト。レスポンスの形を OpenAPI のスキーマで
//! 検査し、開始した受講権が `GET /entitlements/me` で見えること・境界間イベント `SubscriptionStarted` と行動イベント
//! `subscription.started`・2 回目の 409・ゴルフの 422・生徒以外の 403 を確かめる。

use std::time::{Duration, SystemTime};

use application::{
    analytics::AnalyticsEvent,
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo},
    domain_event::DomainEvent,
    entitlement::{
        EntitlementQuotaGate, EntitlementReviewGate, GetEntitlement, StartSubscription,
        subscription_id_uuid,
    },
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
        CancelBooking, ConfirmBooking, CreateBooking, GetSlotSupply, PublishSlot,
        SchedulingBookingLookup, SearchSlots,
    },
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{AccountId, Role, Sport};
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

const STUDENT: &str = "student:1";

/// 2026-10-26T00:00:00Z（現在時刻）
fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
}

struct App {
    router: Router,
    entitlements: InMemoryEntitlementRepository,
    analytics: RecordingAnalyticsSink,
    events: RecordingDomainEventPublisher,
}

fn app() -> App {
    let clock = FixedClock(now());
    let analytics = RecordingAnalyticsSink::default();
    let events = RecordingDomainEventPublisher::default();
    let entitlements = InMemoryEntitlementRepository::default();
    let slots = InMemorySlotRepository::default();
    let bookings = InMemoryBookingRepository::new(slots.clone());
    let router = interface::router_with(
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            analytics.clone(),
            clock,
        ),
        application::identity::ListCoaches::new(InMemoryAccountRepository::default()),
        application::identity::GetAccount::new(InMemoryAccountRepository::default()),
        application::identity::UpdateCoachProfile::new(InMemoryAccountRepository::default()),
        PublishSlot::new(slots.clone(), analytics.clone(), clock),
        SearchSlots::new(
            slots,
            application::identity::IdentityCoachIntroductionLookup::new(
                InMemoryAccountRepository::default(),
            ),
            clock,
        ),
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
        application::scheduling::ListMyBookings::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            application::identity::IdentityCoachIntroductionLookup::new(
                InMemoryAccountRepository::default(),
            ),
        ),
        application::scheduling::ListCoachBookings::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            application::identity::IdentityStudentDisplayNameLookup::new(
                InMemoryAccountRepository::default(),
            ),
        ),
        application::scheduling::ListCoachSlots::new(
            InMemorySlotRepository::default(),
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
        ),
        StartSubscription::new(
            entitlements.clone(),
            analytics.clone(),
            events.clone(),
            clock,
        ),
        GetEntitlement::new(entitlements.clone(), clock),
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
            FixedAiFeedbackGenerator,
            RecordingAnalyticsSink::default(),
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
            application::identity::IdentityStudentDisplayNameLookup::new(
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
        entitlements,
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

/// `POST /subscriptions` を送り、レスポンスの形を契約で検査する。
async fn post_subscriptions(
    router: &Router,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut request =
        Request::post("/subscriptions").header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let (status, body) = send(router, request.body(Body::from(body.to_string())).unwrap()).await;
    support::Contract::load("student-app").assert_response("post", "/subscriptions", status, &body);
    (status, body)
}

fn tennis() -> Value {
    json!({ "sport": "tennis", "payment_method_token": "tok_mock" })
}

#[tokio::test]
async fn student_starts_a_monthly_subscription_with_the_contract_shape() {
    let app = app();

    let (status, body) = post_subscriptions(&app.router, Some(STUDENT), tennis()).await;

    assert_eq!(status, StatusCode::CREATED, "{body}");
    let student_id = account_id_uuid::format(AccountId::from_u128(1));
    let subscription = &body["subscription"];
    assert_eq!(subscription["student_id"], student_id);
    assert_eq!(subscription["sport"], "tennis");
    assert_eq!(subscription["status"], "active");
    assert_eq!(subscription["started_at"], "2026-10-26T00:00:00Z");
    let entitlement = &body["entitlement"];
    assert_eq!(entitlement["student_id"], student_id);
    assert_eq!(entitlement["contract_type"], "subscription");
    assert_eq!(entitlement["unlimited_video_and_chat"], true);
    assert_eq!(
        entitlement["live_lesson_quota"],
        json!({ "month": "2026-10", "limit": 2, "used": 0, "remaining": 2 })
    );

    // 開始した受講権は照会できる
    let (status, me) = send(
        &app.router,
        Request::get("/entitlements/me")
            .header(header::AUTHORIZATION, format!("Bearer {STUDENT}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(&me, entitlement);

    let saved = app.entitlements.subscriptions();
    assert_eq!(saved.len(), 1);
    assert_eq!(
        subscription["subscription_id"],
        subscription_id_uuid::format(saved[0].id())
    );

    let events = app.events.events();
    let [
        DomainEvent::SubscriptionStarted {
            student_id: event_student,
            subscription_id,
            occurred_at,
            ..
        },
    ] = events.as_slice()
    else {
        panic!("SubscriptionStarted が 1 件でない: {events:?}");
    };
    assert_eq!(
        (*event_student, *subscription_id, *occurred_at),
        (AccountId::from_u128(1), saved[0].id(), now())
    );
    assert_eq!(
        app.analytics.events(),
        vec![AnalyticsEvent::SubscriptionStarted {
            student_id: AccountId::from_u128(1),
            subscription_id: saved[0].id(),
            sport: Sport::Tennis,
            occurred_at: now(),
        }]
    );
}

#[tokio::test]
async fn second_subscription_is_409_subscription_already_active() {
    let app = app();
    let (status, _) = post_subscriptions(&app.router, Some(STUDENT), tennis()).await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, body) = post_subscriptions(
        &app.router,
        Some(STUDENT),
        json!({ "sport": "soccer", "payment_method_token": "tok_mock" }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "subscription_already_active");
    assert_eq!(app.entitlements.all().len(), 1);
    assert_eq!(app.events.events().len(), 1);
    assert_eq!(app.analytics.events().len(), 1);
}

#[tokio::test]
async fn golf_and_invalid_requests_are_422_validation_failed() {
    let app = app();
    for body in [
        json!({ "sport": "golf", "payment_method_token": "tok_mock" }),
        json!({ "sport": "cricket", "payment_method_token": "tok_mock" }),
        json!({ "sport": "tennis" }),
        json!({}),
    ] {
        let (status, response) = post_subscriptions(&app.router, Some(STUDENT), body.clone()).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{body}: {response}"
        );
        assert_eq!(response["code"], "validation_failed");
    }
    assert!(app.entitlements.all().is_empty());
    assert!(app.events.events().is_empty());
}

#[tokio::test]
async fn only_authenticated_students_can_start_a_subscription() {
    let app = app();
    for token in ["coach:1", "headquarters:1"] {
        let (status, body) = post_subscriptions(&app.router, Some(token), tennis()).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
    }
    let (status, body) = post_subscriptions(&app.router, None, tennis()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert!(app.entitlements.all().is_empty());
}
