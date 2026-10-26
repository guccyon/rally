//! `POST /accounts` の統合テスト。レスポンスの形を student-app の OpenAPI
//! （specs/contracts/api/student-app/openapi.yaml）のスキーマで検査する。

use std::time::{Duration, SystemTime};

use application::{
    analytics::AnalyticsEvent,
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo},
    entitlement::{EntitlementQuotaGate, EntitlementReviewGate, GetEntitlement, StartSubscription},
    identity::{
        AccessToken, InvalidToken, RegisterAccount, TokenClaims, TokenIssuer, TokenVerifier,
        account_id_uuid,
    },
    in_memory::{
        FixedClock, FixedVideoCallProvider, FixedVideoStorage, InMemoryAccountRepository,
        InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemoryLiveLessonSessionRepository, InMemorySlotRepository, InMemoryVideoRepository,
        RecordingAnalyticsSink, RecordingDomainEventPublisher,
    },
    live_lesson::{EndLiveLessonSession, StartLiveLessonSession},
    scheduling::{
        CancelBooking, ConfirmBooking, CreateBooking, PublishSlot, SchedulingBookingLookup,
        SearchSlots,
    },
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{Role, Sport};
use serde_json::{Value, json};
use tower::ServiceExt;

mod support;

/// 2026-10-26T00:00:00Z
const NOW: Duration = Duration::from_secs(1_792_972_800);

/// 発行した内容が読める形のトークン（interface は infra のモックに依存できないので、テスト用に置く）。
struct FakeTokens;

impl TokenIssuer for FakeTokens {
    fn issue(&self, claims: TokenClaims) -> AccessToken {
        AccessToken::new(format!(
            "fake:{:?}:{}",
            claims.role,
            account_id_uuid::format(claims.account_id)
        ))
    }
}

impl TokenVerifier for FakeTokens {
    fn verify(&self, _token: &str) -> Result<TokenClaims, InvalidToken> {
        Err(InvalidToken)
    }
}

struct App {
    router: Router,
    analytics: RecordingAnalyticsSink,
}

fn app() -> App {
    let analytics = RecordingAnalyticsSink::default();
    let clock = FixedClock(SystemTime::UNIX_EPOCH + NOW);
    let slots = InMemorySlotRepository::default();
    let bookings = InMemoryBookingRepository::new(slots.clone());
    let router = interface::router_with(
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
        FakeTokens,
    );
    App { router, analytics }
}

async fn post_accounts(router: &Router, body: String) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(
            Request::post("/accounts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/json",
        "{status}"
    );
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn request(email: &str, display_name: &str, sport: &str) -> String {
    json!({ "email": email, "display_name": display_name, "sport": sport }).to_string()
}

fn assert_contract(status: StatusCode, body: &Value) {
    support::Contract::load("student-app").assert_response("post", "/accounts", status, body);
}

// --- テスト ---

#[tokio::test]
async fn registers_a_student_with_the_contract_shape() {
    let app = app();

    let (status, body) =
        post_accounts(&app.router, request("taro@example.com", "たろう", "tennis")).await;

    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_contract(status, &body);
    let student = &body["student"];
    assert_eq!(student["display_name"], "たろう");
    assert_eq!(student["sport"], "tennis");
    assert_eq!(student["created_at"], "2026-10-26T00:00:00Z");
    assert_eq!(
        body["access_token"],
        format!(
            "fake:{:?}:{}",
            Role::Student,
            student["student_id"].as_str().unwrap()
        ),
        "トークンは TokenIssuer が生徒の ID と役割で発行する"
    );

    let events = app.analytics.events();
    assert_eq!(events.len(), 1, "account.registered は 1 回だけ発火する");
    let AnalyticsEvent::AccountRegistered {
        student_id,
        sport,
        occurred_at,
    } = &events[0]
    else {
        panic!("{:?}", events[0]);
    };
    assert_eq!(events[0].name(), "account.registered");
    assert_eq!(account_id_uuid::format(*student_id), student["student_id"]);
    assert_eq!(*sport, Sport::Tennis);
    assert_eq!(*occurred_at, SystemTime::UNIX_EPOCH + NOW);
}

#[tokio::test]
async fn accepts_every_student_app_sport() {
    let app = app();
    for (i, sport) in ["tennis", "baseball", "soccer", "basketball"]
        .iter()
        .enumerate()
    {
        let (status, body) = post_accounts(
            &app.router,
            request(&format!("s{i}@example.com"), "生徒", sport),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{sport}: {body}");
        assert_eq!(body["student"]["sport"], *sport);
    }
}

#[tokio::test]
async fn invalid_input_is_422_validation_failed_without_emitting() {
    let app = app();
    let cases = [
        (
            "未知のスポーツ",
            request("taro@example.com", "たろう", "cricket"),
        ),
        (
            "ゴルフ（golf-app の範囲）",
            request("taro@example.com", "たろう", "golf"),
        ),
        ("空の表示名", request("taro@example.com", "", "tennis")),
        (
            "空白だけの表示名",
            request("taro@example.com", "  ", "tennis"),
        ),
        (
            "51 文字の表示名",
            request("taro@example.com", &"あ".repeat(51), "tennis"),
        ),
        ("メールアドレスの形", request("taro", "たろう", "tennis")),
        (
            "必須項目の欠落",
            json!({ "email": "taro@example.com", "sport": "tennis" }).to_string(),
        ),
        (
            "型の違い",
            json!({ "email": "taro@example.com", "display_name": "たろう", "sport": 1 })
                .to_string(),
        ),
        ("JSON でない", "{".to_string()),
    ];

    for (case, body) in cases {
        let (status, body) = post_accounts(&app.router, body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{case}: {body}");
        assert_contract(status, &body);
        assert_eq!(body["code"], "validation_failed", "{case}");
    }
    assert!(app.analytics.events().is_empty());
}

#[tokio::test]
async fn duplicate_email_is_409_email_already_registered() {
    let app = app();
    let (status, _) =
        post_accounts(&app.router, request("taro@example.com", "たろう", "tennis")).await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, body) =
        post_accounts(&app.router, request("Taro@Example.com", "たろう", "soccer")).await;

    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_contract(status, &body);
    assert_eq!(body["code"], "email_already_registered");
    assert_eq!(app.analytics.events().len(), 1);
}
