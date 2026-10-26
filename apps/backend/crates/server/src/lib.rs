//! 組み立て用のクレート（composition root）。interface のルーターに、ポートの実装を渡す。
//!
//! interface と infra の両方に依存してよいのはこのクレートだけ（docs/adr/0003-backend-layers.md の決定 5）。

use application::{
    clock::SystemClock,
    identity::RegisterAccount,
    in_memory::{InMemoryAccountRepository, RecordingAnalyticsSink},
};
use axum::Router;
use infra::MockTokenService;

/// HTTP のルーター。アカウントの保存先と行動トラッキングは当面インメモリ、アクセストークンは
/// infra のモック（署名なし。本番では差し替える）で組み立てる。
pub fn router() -> Router {
    interface::router_with(
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            RecordingAnalyticsSink::default(),
            SystemClock,
        ),
        MockTokenService,
    )
}
