//! インターフェース層。axum の HTTP ルーティングを置く。
//!
//! 依存してよいのは application と domain。infra には直接依存しない
//! （docs/adr/0003-backend-layers.md）。ポートの実装は組み立て用のクレート（`server`）が渡す。

mod accounts;
pub mod auth;
mod contract;
mod error;
mod message;
mod slots;

use std::sync::Arc;

use application::{
    analytics::AnalyticsSink,
    clock::Clock,
    identity::{AccountRepository, RegisterAccount, TokenIssuer, TokenVerifier},
    scheduling::{PublishSlot, SearchSlots, SlotRepository},
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
pub fn router_with<R, S, A, C, T>(
    register_account: RegisterAccount<R, A, C>,
    publish_slot: PublishSlot<S, A, C>,
    search_slots: SearchSlots<S, C>,
    tokens: T,
) -> Router
where
    R: AccountRepository + 'static,
    S: SlotRepository + 'static,
    A: AnalyticsSink + 'static,
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
    Router::new()
        .route("/healthz", get(healthz))
        .merge(accounts)
        .merge(slots)
        .layer(auth::TokenVerification::layer(tokens))
}

/// 死活監視。プロセスが要求を受け付けられれば 200 を返す。
async fn healthz() -> (StatusCode, &'static str) {
    (StatusCode::OK, "ok")
}
