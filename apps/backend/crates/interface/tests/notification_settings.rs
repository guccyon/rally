//! `GET /notification-settings/me`（student-app の `getMyNotificationSettings`）と `PUT /notification-settings/me`
//! （`updateMyNotificationSettings`）の統合テスト。レスポンスの形を OpenAPI のスキーマで検査し、既定がすべて true・
//! 書いた種類だけが変わること・ほかの生徒に触れないこと・真偽値でない値の 422・生徒以外の 403・トークンなしの 401 を
//! 確かめる。

use std::time::SystemTime;

use application::{
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo},
    entitlement::{EntitlementQuotaGate, EntitlementReviewGate, GetEntitlement, StartSubscription},
    identity::{
        AccessToken, InvalidToken, RegisterAccount, TokenClaims, TokenIssuer, TokenVerifier,
        account_id_uuid,
    },
    in_memory::{
        FixedAiFeedbackGenerator, FixedClock, FixedVideoCallProvider, FixedVideoStorage,
        InMemoryAccountRepository, InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemoryLessonCompletionRepository, InMemoryLiveLessonSessionRepository,
        InMemoryNotificationSettingsRepository, InMemorySlotRepository, InMemoryVideoRepository,
        RecordingAnalyticsSink, RecordingDomainEventPublisher,
    },
    lesson::RecordLessonCompletion,
    live_lesson::{EndLiveLessonSession, StartLiveLessonSession},
    notification::{GetNotificationSettings, UpdateNotificationSettings},
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
    identity::{AccountId, Role},
    notification::{NotificationKind, NotificationSettings},
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

const STUDENT: &str = "student:1";
const OTHER_STUDENT: &str = "student:2";

/// 受け取り設定の保存先を `settings` にしたルーター。
fn router(settings: InMemoryNotificationSettingsRepository) -> Router {
    let clock = FixedClock(SystemTime::UNIX_EPOCH);
    let analytics = RecordingAnalyticsSink::default();
    let entitlements = InMemoryEntitlementRepository::default();
    let slots = InMemorySlotRepository::default();
    let bookings = InMemoryBookingRepository::new(slots.clone());
    interface::router_with(
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
        CreateBooking::new(bookings.clone(), analytics.clone(), clock),
        ConfirmBooking::new(
            bookings.clone(),
            EntitlementQuotaGate::new(entitlements.clone()),
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
            entitlements.clone(),
            analytics.clone(),
            RecordingDomainEventPublisher::default(),
            clock,
        ),
        GetEntitlement::new(entitlements.clone(), clock),
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
            FixedVideoStorage,
            FixedAiFeedbackGenerator,
            analytics.clone(),
            RecordingDomainEventPublisher::default(),
            clock,
        ),
        GetVideoReview::new(InMemoryVideoRepository::default()),
        ReturnVideoReview::new(
            InMemoryVideoRepository::default(),
            EntitlementReviewGate,
            analytics.clone(),
            RecordingDomainEventPublisher::default(),
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
            analytics.clone(),
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
            application::entitlement::EntitlementChatGate::new(entitlements),
            analytics.clone(),
            RecordingDomainEventPublisher::default(),
            clock,
        ),
        application::messaging::MarkMessageThreadRead::new(
            application::in_memory::InMemoryMessageThreadRepository::default(),
            clock,
        ),
        application::review::SubmitLessonReview::new(
            application::lesson::LessonCompletionLookup::new(
                InMemoryLessonCompletionRepository::default(),
            ),
            application::in_memory::InMemoryLessonReviewRepository::default(),
            analytics,
            clock,
        ),
        application::review::GetCoachReviewSummary::new(
            application::in_memory::InMemoryLessonReviewRepository::default(),
        ),
        application::review::ListCoachReviews::new(
            application::in_memory::InMemoryLessonReviewRepository::default(),
        ),
        GetNotificationSettings::new(settings.clone()),
        UpdateNotificationSettings::new(settings),
        FakeTokens,
    )
}

/// `method /notification-settings/me` を送り、レスポンスの形を契約で検査する。`body` は PUT のリクエスト（文字列のまま送る）。
async fn call(
    router: &Router,
    method: &str,
    token: Option<&str>,
    body: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri("/notification-settings/me")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let request = request
        .body(body.map_or_else(Body::empty, |b| Body::from(b.to_owned())))
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    support::Contract::load("student-app").assert_response(
        &method.to_lowercase(),
        "/notification-settings/me",
        status,
        &body,
    );
    (status, body)
}

fn student_id(id: u128) -> String {
    account_id_uuid::format(AccountId::from_u128(id))
}

#[tokio::test]
async fn a_student_who_has_not_changed_anything_gets_every_kind_on() {
    let settings = InMemoryNotificationSettingsRepository::default();
    let (status, body) = call(&router(settings.clone()), "GET", Some(STUDENT), None).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "student_id": student_id(1),
            "booking_confirmed": true,
            "video_review_returned": true,
            "message_received": true,
        })
    );
    assert!(settings.all().is_empty(), "照会では保存しない");
}

#[tokio::test]
async fn put_changes_only_the_written_kinds_of_the_caller() {
    let settings = InMemoryNotificationSettingsRepository::default();
    let router = router(settings.clone());

    let (status, body) = call(
        &router,
        "PUT",
        Some(STUDENT),
        Some(r#"{ "message_received": false }"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "student_id": student_id(1),
            "booking_confirmed": true,
            "video_review_returned": true,
            "message_received": false,
        })
    );

    // 書かなかった種類（メッセージ）は今の設定のまま。空のオブジェクトは何も変えない
    let (status, body) = call(
        &router,
        "PUT",
        Some(STUDENT),
        Some(r#"{ "booking_confirmed": false, "video_review_returned": true }"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, unchanged) = call(&router, "PUT", Some(STUDENT), Some("{}")).await;
    let (_, read) = call(&router, "GET", Some(STUDENT), None).await;
    for body in [&body, &unchanged, &read] {
        assert_eq!(
            body,
            &json!({
                "student_id": student_id(1),
                "booking_confirmed": false,
                "video_review_returned": true,
                "message_received": false,
            })
        );
    }
    assert_eq!(
        settings.all(),
        [NotificationSettings::restore(
            AccountId::from_u128(1),
            [
                NotificationKind::BookingConfirmed,
                NotificationKind::MessageReceived
            ]
        )]
    );

    // ほかの生徒は既定のまま
    let (_, other) = call(&router, "GET", Some(OTHER_STUDENT), None).await;
    assert_eq!(other["message_received"], true);
}

#[tokio::test]
async fn a_value_that_is_not_a_boolean_is_422() {
    let settings = InMemoryNotificationSettingsRepository::default();
    let router = router(settings.clone());
    for body in [
        r#"{ "message_received": "off" }"#,
        r#"{ "booking_confirmed": 0 }"#,
        "[]",
        "not json",
    ] {
        let (status, error) = call(&router, "PUT", Some(STUDENT), Some(body)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}: {error}");
        assert_eq!(error["code"], "validation_failed");
    }
    assert!(settings.all().is_empty(), "何も保存しない");
}

#[tokio::test]
async fn only_students_can_call_and_a_token_is_required() {
    let router = router(InMemoryNotificationSettingsRepository::default());
    for token in ["coach:9", "headquarters:7"] {
        for (method, body) in [("GET", None), ("PUT", Some("{}"))] {
            let (status, error) = call(&router, method, Some(token), body).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{method} {token}: {error}");
            assert_eq!(error["code"], "forbidden");
        }
    }
    for (method, body) in [("GET", None), ("PUT", Some("{}"))] {
        let (status, error) = call(&router, method, None, body).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method}: {error}");
    }
}
