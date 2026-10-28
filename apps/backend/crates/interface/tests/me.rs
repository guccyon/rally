//! `GET /me`（admin-web の `getMyAccount`）の統合テスト。レスポンスの形を admin-web の OpenAPI のスキーマで
//! 検査し、トークンが示す本人のアカウント（ID・役割・表示名）を返すことと、トークンがなければ 401 であることを確かめる。

use std::time::SystemTime;

use application::{
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo},
    entitlement::{EntitlementQuotaGate, EntitlementReviewGate, GetEntitlement, StartSubscription},
    identity::{
        AccessToken, AccountRepository, GetAccount, InvalidToken, ListCoaches, RegisterAccount,
        TokenClaims, TokenIssuer, TokenVerifier,
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
        SchedulingBookingLookup, SearchSlots,
    },
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{Account, AccountId, DisplayName, Email, Role, Sport};
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

/// 本部・コーチ・生徒が 1 人ずつ。
async fn accounts() -> InMemoryAccountRepository {
    let accounts = InMemoryAccountRepository::default();
    for account in [
        account(1, "本部", Role::Headquarters, &[]),
        account(2, "たなか", Role::Coach, &[Sport::Tennis]),
        account(3, "あおき", Role::Student, &[Sport::Tennis]),
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
        GetAccount::new(accounts),
        PublishSlot::new(slots.clone(), analytics.clone(), clock),
        SearchSlots::new(slots, clock),
        CreateBooking::new(bookings.clone(), analytics.clone(), clock),
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
            FixedVideoStorage,
            analytics.clone(),
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
        ),
        FakeTokens,
    )
}

async fn get_my_account(router: &Router, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get("/me");
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
    support::Contract::load("admin-web").assert_response("get", "/me", status, &body);
    (status, body)
}

#[tokio::test]
async fn coach_gets_own_account() {
    let router = app(accounts().await);
    let (status, body) = get_my_account(&router, Some("coach:2")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "account_id": "00000000-0000-0000-0000-000000000002",
            "role": "coach",
            "display_name": "たなか",
        })
    );
}

#[tokio::test]
async fn headquarters_gets_own_account() {
    let router = app(accounts().await);
    let (status, body) = get_my_account(&router, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "account_id": "00000000-0000-0000-0000-000000000001",
            "role": "headquarters",
            "display_name": "本部",
        })
    );
}

#[tokio::test]
async fn student_token_is_not_forbidden() {
    let router = app(accounts().await);
    let (status, body) = get_my_account(&router, Some("student:3")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["role"], "student");
}

#[tokio::test]
async fn missing_or_invalid_token_is_unauthorized() {
    let router = app(accounts().await);
    for token in [None, Some("nobody:1")] {
        let (status, body) = get_my_account(&router, token).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}: {body}");
        assert_eq!(body["code"], "unauthorized");
    }
}

#[tokio::test]
async fn token_of_unknown_account_or_other_role_is_unauthorized() {
    let router = app(accounts().await);
    // 99 はアカウントがない。2 はコーチ、3 は生徒で、トークンの役割と合わない
    for token in ["coach:99", "headquarters:2", "coach:3"] {
        let (status, body) = get_my_account(&router, Some(token)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token}: {body}");
        assert_eq!(body["code"], "unauthorized");
    }
}
