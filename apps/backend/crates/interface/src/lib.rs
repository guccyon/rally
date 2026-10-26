//! インターフェース層。axum の HTTP ルーティングを置く。
//!
//! 依存してよいのは application と domain。infra には直接依存しない
//! （docs/adr/0003-backend-layers.md）。ポートの実装は組み立て用のクレート（`server`）が渡す。

mod accounts;
pub mod auth;
mod bookings;
mod contract;
mod entitlements;
mod error;
mod message;
mod slots;
mod subscriptions;

use std::sync::Arc;

use application::{
    analytics::AnalyticsSink,
    clock::Clock,
    domain_event::DomainEventPublisher,
    entitlement::{EntitlementRepository, GetEntitlement, StartSubscription},
    identity::{AccountRepository, RegisterAccount, TokenIssuer, TokenVerifier},
    scheduling::{
        BookingRepository, CancelBooking, ConfirmBooking, CreateBooking, PublishSlot, QuotaGate,
        SearchSlots, SlotRepository,
    },
};
use axum::{
    Router,
    http::StatusCode,
    routing::{get, post},
};
use domain::identity::Role;

pub use auth::{AuthenticatedAccount, require_role};
pub use error::ApiError;

/// ユースケースとアクセストークンのポート（発行・検証）を外から渡して組み立てるルーター。
///
/// `/healthz` と `/accounts` 以外のルートは認証必須（[`AuthenticatedAccount`] を取る）。
/// `POST /slots`（admin-web）はコーチだけ、`GET /slots`（student-app）は認証した利用者なら誰でも呼べる。
/// `/bookings`（student-app の予約の作成・確定・キャンセル）・`POST /subscriptions`（月額契約の開始）・
/// `GET /entitlements/me`（受講権の照会）は生徒だけ。
#[allow(clippy::too_many_arguments)]
pub fn router_with<R, S, B, G, E, A, P, C, T>(
    register_account: RegisterAccount<R, A, C>,
    publish_slot: PublishSlot<S, A, C>,
    search_slots: SearchSlots<S, C>,
    create_booking: CreateBooking<B, A, C>,
    confirm_booking: ConfirmBooking<B, G, A, P, C>,
    cancel_booking: CancelBooking<B, A, P, C>,
    start_subscription: StartSubscription<E, A, P, C>,
    get_entitlement: GetEntitlement<E, C>,
    tokens: T,
) -> Router
where
    R: AccountRepository + 'static,
    S: SlotRepository + 'static,
    B: BookingRepository + 'static,
    G: QuotaGate + 'static,
    E: EntitlementRepository + 'static,
    A: AnalyticsSink + 'static,
    P: DomainEventPublisher + 'static,
    C: Clock + 'static,
    T: TokenIssuer + TokenVerifier + 'static,
{
    let tokens = Arc::new(tokens);
    let accounts = Router::new()
        .route("/accounts", post(accounts::register_account::<R, A, C>))
        .with_state(Arc::new(accounts::AccountsState {
            register_account,
            tokens: tokens.clone(),
        }));
    let slots = Router::new()
        .route(
            "/slots",
            get(slots::search_slots::<S, A, C>).merge(
                post(slots::publish_slot::<S, A, C>).route_layer(require_role(&[Role::Coach])),
            ),
        )
        .with_state(Arc::new(slots::SlotsState {
            publish_slot,
            search_slots,
        }));
    let bookings = Router::new()
        .route("/bookings", post(bookings::create_booking::<B, G, A, P, C>))
        .route(
            "/bookings/{booking_id}/confirm",
            post(bookings::confirm_booking::<B, G, A, P, C>),
        )
        .route(
            "/bookings/{booking_id}/cancel",
            post(bookings::cancel_booking::<B, G, A, P, C>),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(bookings::BookingsState {
            create_booking,
            confirm_booking,
            cancel_booking,
        }));
    let entitlements = Router::new()
        .route(
            "/entitlements/me",
            get(entitlements::get_my_entitlement::<E, C>),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(entitlements::EntitlementsState {
            get_entitlement,
        }));
    let subscriptions = Router::new()
        .route(
            "/subscriptions",
            post(subscriptions::start_subscription::<E, A, P, C>),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(subscriptions::SubscriptionsState {
            start_subscription,
        }));
    Router::new()
        .route("/healthz", get(healthz))
        .merge(accounts)
        .merge(slots)
        .merge(bookings)
        .merge(subscriptions)
        .merge(entitlements)
        .layer(auth::TokenVerification::layer(tokens))
}

/// 死活監視。プロセスが要求を受け付けられれば 200 を返す。
async fn healthz() -> (StatusCode, &'static str) {
    (StatusCode::OK, "ok")
}
