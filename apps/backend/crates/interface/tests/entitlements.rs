//! `GET /entitlements/me`（student-app の `getMyEntitlement`）の統合テスト。レスポンスの形を OpenAPI の
//! スキーマで検査し、月をまたいだら通話の枠の `used` が 0 から数えられること・ワンショットの形・
//! 受講権がないときの 404・生徒以外の 403 を確かめる。

use std::time::{Duration, SystemTime};

use application::{
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo},
    entitlement::{
        EntitlementQuotaGate, EntitlementReviewGate, GetEntitlement, StartSubscription,
        entitlement_id_uuid,
    },
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
        SchedulingBookingLookup, SearchSlots,
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
    scheduling::BookingId,
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

/// 月額（テニス）の生徒。10 月の通話の枠を使い切っている
const MONTHLY_STUDENT: &str = "student:1";
/// ワンショット（ゴルフ）の生徒
const ONE_SHOT_STUDENT: &str = "student:2";
/// 受講権のない生徒
const NEW_STUDENT: &str = "student:3";

/// 2026-11-01T00:00:00+09:00（暦月は日本時間で区切る）
fn nov1() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_458_800)
}

fn monthly() -> Entitlement {
    Entitlement::monthly(
        EntitlementId::from_u128(10),
        AccountId::from_u128(1),
        Sport::Tennis,
        SubscriptionId::from_u128(20),
        LiveLessonQuota::restore(
            YearMonth::new(2026, 10).unwrap(),
            [BookingId::from_u128(1), BookingId::from_u128(2)],
        )
        .unwrap(),
    )
    .unwrap()
}

fn one_shot() -> Entitlement {
    Entitlement::one_shot(
        EntitlementId::from_u128(11),
        AccountId::from_u128(2),
        Sport::Golf,
        1,
    )
    .unwrap()
}

/// 現在時刻を `now` にしたルーター。
fn router(now: SystemTime) -> Router {
    let clock = FixedClock(now);
    let analytics = RecordingAnalyticsSink::default();
    let entitlements = InMemoryEntitlementRepository::with_entitlements([monthly(), one_shot()]);
    let slots = InMemorySlotRepository::default();
    let bookings = InMemoryBookingRepository::new(slots.clone());
    interface::router_with(
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            analytics.clone(),
            clock,
        ),
        PublishSlot::new(slots.clone(), analytics.clone(), clock),
        SearchSlots::new(slots, clock),
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
        StartSubscription::new(
            entitlements.clone(),
            analytics,
            RecordingDomainEventPublisher::default(),
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
        FakeTokens,
    )
}

/// `GET /entitlements/me` を送り、レスポンスの形を契約で検査する。
async fn get_my_entitlement(router: &Router, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get("/entitlements/me");
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
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    support::Contract::load("student-app").assert_response(
        "get",
        "/entitlements/me",
        status,
        &body,
    );
    (status, body)
}

#[tokio::test]
async fn returns_the_monthly_entitlement_with_the_quota_of_the_current_month() {
    let (status, body) = get_my_entitlement(
        &router(nov1() - Duration::from_secs(1)),
        Some(MONTHLY_STUDENT),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "entitlement_id": entitlement_id_uuid::format(EntitlementId::from_u128(10)),
            "student_id": account_id_uuid::format(AccountId::from_u128(1)),
            "contract_type": "subscription",
            "sport": "tennis",
            "unlimited_video_and_chat": true,
            "live_lesson_quota": { "month": "2026-10", "limit": 2, "used": 2, "remaining": 0 },
            "remaining_video_reviews": null,
        })
    );
}

#[tokio::test]
async fn crossing_the_month_resets_used_to_0() {
    let router = router(nov1());

    let (status, body) = get_my_entitlement(&router, Some(MONTHLY_STUDENT)).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["live_lesson_quota"],
        json!({ "month": "2026-11", "limit": 2, "used": 0, "remaining": 2 }),
        "10 月を使い切っていても 11 月は 0 から"
    );
}

#[tokio::test]
async fn returns_the_one_shot_entitlement_without_a_live_lesson_quota() {
    let (status, body) = get_my_entitlement(&router(nov1()), Some(ONE_SHOT_STUDENT)).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["contract_type"], "one_shot");
    assert_eq!(body["sport"], "golf");
    assert_eq!(body["unlimited_video_and_chat"], false);
    assert_eq!(body["live_lesson_quota"], Value::Null);
    assert_eq!(body["remaining_video_reviews"], 1);
}

#[tokio::test]
async fn student_without_an_entitlement_gets_404() {
    let (status, body) = get_my_entitlement(&router(nov1()), Some(NEW_STUDENT)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["code"], "not_found");
}

#[tokio::test]
async fn only_students_can_call() {
    let router = router(nov1());
    for token in ["coach:1", "headquarters:1"] {
        let (status, body) = get_my_entitlement(&router, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
    }
    let (status, body) = get_my_entitlement(&router, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "unauthorized");
}
