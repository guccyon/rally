//! インターフェース層。axum の HTTP ルーティングを置く。
//!
//! 依存してよいのは application と domain。infra には直接依存しない
//! （docs/adr/0003-backend-layers.md）。

mod accounts;
mod error;

use std::sync::Arc;

use application::{
    analytics::AnalyticsSink,
    clock::{Clock, SystemClock},
    identity::{AccountRepository, RegisterAccount},
    in_memory::{InMemoryAccountRepository, RecordingAnalyticsSink},
};
use axum::{
    Router,
    http::StatusCode,
    routing::{get, post},
};

/// HTTP のルーター。ポートはインメモリの実装で組み立てる（infra の実装を組み立てる
/// composition root ができるまで。ADR 0003 の決定 5）。
pub fn router() -> Router {
    router_with(RegisterAccount::new(
        InMemoryAccountRepository::default(),
        RecordingAnalyticsSink::default(),
        SystemClock,
    ))
}

/// ユースケースを外から渡して組み立てるルーター。
pub fn router_with<R, A, C>(register_account: RegisterAccount<R, A, C>) -> Router
where
    R: AccountRepository + 'static,
    A: AnalyticsSink + 'static,
    C: Clock + 'static,
{
    Router::new()
        .route("/healthz", get(healthz))
        .route("/accounts", post(accounts::register_account::<R, A, C>))
        .with_state(Arc::new(register_account))
}

/// 死活監視。プロセスが要求を受け付けられれば 200 を返す。
async fn healthz() -> (StatusCode, &'static str) {
    (StatusCode::OK, "ok")
}
