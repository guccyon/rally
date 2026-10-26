//! インターフェース層。axum の HTTP ルーティングを置く。
//!
//! 依存してよいのは application と domain。infra には直接依存しない
//! （docs/adr/0003-backend-layers.md）。ポートの実装は組み立て用のクレート（`server`）が渡す。

mod accounts;
pub mod auth;
mod contract;
mod error;
mod message;

use std::sync::Arc;

use application::{
    analytics::AnalyticsSink,
    clock::Clock,
    identity::{AccountRepository, RegisterAccount, TokenIssuer, TokenVerifier},
};
use axum::{
    Router,
    http::StatusCode,
    routing::{get, post},
};

pub use auth::{AuthenticatedAccount, require_role};
pub use error::ApiError;

/// ユースケースとアクセストークンのポート（発行・検証）を外から渡して組み立てるルーター。
///
/// `/healthz` と `/accounts` 以外のルートは認証必須（[`AuthenticatedAccount`] を取る）。
pub fn router_with<R, A, C, T>(register_account: RegisterAccount<R, A, C>, tokens: T) -> Router
where
    R: AccountRepository + 'static,
    A: AnalyticsSink + 'static,
    C: Clock + 'static,
    T: TokenIssuer + TokenVerifier + 'static,
{
    let tokens = Arc::new(tokens);
    Router::new()
        .route("/healthz", get(healthz))
        .route("/accounts", post(accounts::register_account::<R, A, C>))
        .with_state(Arc::new(accounts::AccountsState {
            register_account,
            tokens: tokens.clone(),
        }))
        .layer(auth::TokenVerification::layer(tokens))
}

/// 死活監視。プロセスが要求を受け付けられれば 200 を返す。
async fn healthz() -> (StatusCode, &'static str) {
    (StatusCode::OK, "ok")
}
