//! 組み立て用のクレート（composition root）。interface のルーターに、ポートの実装を渡す。
//!
//! interface と infra の両方に依存してよいのはこのクレートだけ（docs/adr/0003-backend-layers.md の決定 5）。

use application::{
    clock::SystemClock,
    entitlement::GetEntitlement,
    identity::RegisterAccount,
    in_memory::{
        InMemoryAccountRepository, InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemorySlotRepository, RecordingAnalyticsSink,
    },
    scheduling::{CancelBooking, ConfirmBooking, CreateBooking, PublishSlot, SearchSlots},
};
use axum::Router;
use infra::{LoggingDomainEventPublisher, MockTokenService};

/// HTTP のルーター。アカウント・スロット・予約の保存先と行動トラッキングは当面インメモリ、アクセストークンは
/// infra のモック（署名なし。本番では差し替える）、境界間イベントの発行は infra のモック（標準出力に書くだけ）で
/// 組み立てる。受講権の保存先も当面インメモリ（受講権を作る月額契約の開始は T11 なので、照会は 404 になる）。
///
/// スロットの Postgres 実装（`infra::PgSlotRepository`）は `scheduling.slots` の外部キーが
/// `identity.coaches` を指すので、アカウントの保存先を Postgres にするときに一緒に差し替える（予約も同じ）。
pub fn router() -> Router {
    let analytics = RecordingAnalyticsSink::default();
    let slots = InMemorySlotRepository::default();
    let bookings = InMemoryBookingRepository::new(slots.clone());
    interface::router_with(
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            analytics.clone(),
            SystemClock,
        ),
        PublishSlot::new(slots.clone(), analytics.clone(), SystemClock),
        SearchSlots::new(slots, SystemClock),
        CreateBooking::new(bookings.clone(), analytics.clone(), SystemClock),
        ConfirmBooking::new(
            bookings.clone(),
            analytics.clone(),
            LoggingDomainEventPublisher,
            SystemClock,
        ),
        CancelBooking::new(
            bookings,
            analytics,
            LoggingDomainEventPublisher,
            SystemClock,
        ),
        GetEntitlement::new(InMemoryEntitlementRepository::default(), SystemClock),
        MockTokenService,
    )
}
