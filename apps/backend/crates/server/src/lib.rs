//! 組み立て用のクレート（composition root）。interface のルーターに、ポートの実装を渡す。
//!
//! interface と infra の両方に依存してよいのはこのクレートだけ（docs/adr/0003-backend-layers.md の決定 5）。

use application::{
    clock::SystemClock,
    identity::RegisterAccount,
    in_memory::{InMemoryAccountRepository, InMemorySlotRepository, RecordingAnalyticsSink},
    scheduling::{PublishSlot, SearchSlots},
};
use axum::Router;
use infra::MockTokenService;

/// HTTP のルーター。アカウント・スロットの保存先と行動トラッキングは当面インメモリ、アクセストークンは
/// infra のモック（署名なし。本番では差し替える）で組み立てる。
///
/// スロットの Postgres 実装（`infra::PgSlotRepository`）は `scheduling.slots` の外部キーが
/// `identity.coaches` を指すので、アカウントの保存先を Postgres にするときに一緒に差し替える。
pub fn router() -> Router {
    let analytics = RecordingAnalyticsSink::default();
    let slots = InMemorySlotRepository::default();
    interface::router_with(
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            analytics.clone(),
            SystemClock,
        ),
        PublishSlot::new(slots.clone(), analytics, SystemClock),
        SearchSlots::new(slots, SystemClock),
        MockTokenService,
    )
}
