//! 500 応答に内部の理由が出ないことの検査。保存先が障害を返す `POST /accounts` で確かめる。

use std::time::SystemTime;

use application::{
    Unavailable,
    entitlement::{EntitlementQuotaGate, GetEntitlement, StartSubscription},
    identity::{
        AccessToken, AccountRepository, InsertAccountError, InvalidToken, RegisterAccount,
        TokenClaims, TokenIssuer, TokenVerifier,
    },
    in_memory::{
        FixedClock, InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemorySlotRepository, RecordingAnalyticsSink, RecordingDomainEventPublisher,
    },
    scheduling::{CancelBooking, ConfirmBooking, CreateBooking, PublishSlot, SearchSlots},
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
