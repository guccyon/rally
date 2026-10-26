//! 組み立て用のクレート（composition root）。interface のルーターに、ポートの実装を渡す。
//!
//! interface と infra の両方に依存してよいのはこのクレートだけ（docs/adr/0003-backend-layers.md の決定 5）。

use application::{
    clock::{Clock, SystemClock},
    entitlement::{
        EntitlementQuotaGate, GetEntitlement, LiveLessonQuotaSubscriber, StartSubscription,
    },
    identity::RegisterAccount,
    in_memory::{
        InMemoryAccountRepository, InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemorySlotRepository, RecordingAnalyticsSink,
    },
    scheduling::{CancelBooking, ConfirmBooking, CreateBooking, PublishSlot, SearchSlots},
};
use axum::Router;
use infra::{InMemoryDomainEventBus, MockTokenService};

/// HTTP のルーター（現在時刻は OS の時計）。組み立ては [`router_at`]。
pub fn router() -> Router {
    router_at(SystemClock)
}

/// 現在時刻を `clock` から読むルーター。アカウント・スロット・予約・受講権の保存先と行動トラッキングは当面
/// インメモリ、アクセストークンは infra のモック（署名なし。本番では差し替える）で組み立てる。
///
/// 境界間イベントは infra のインメモリのイベントバス（モック。標準出力にも書く）で発行し、entitlement の購読
/// （`LiveLessonQuotaSubscriber`）につなぐ。予約の確定・キャンセルでビデオ通話の暦月の枠が動き、予約の確定は
/// entitlement に枠が残っているかを問い合わせる（`EntitlementQuotaGate`）。
///
/// スロットの Postgres 実装（`infra::PgSlotRepository`）は `scheduling.slots` の外部キーが
/// `identity.coaches` を指すので、アカウントの保存先を Postgres にするときに一緒に差し替える（予約・受講権も同じ）。
pub fn router_at<C>(clock: C) -> Router
where
    C: Clock + Clone + 'static,
{
    let analytics = RecordingAnalyticsSink::default();
    let slots = InMemorySlotRepository::default();
    let bookings = InMemoryBookingRepository::new(slots.clone());
    let entitlements = InMemoryEntitlementRepository::default();
    let events = InMemoryDomainEventBus::new(LiveLessonQuotaSubscriber::new(entitlements.clone()));
    interface::router_with(
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            analytics.clone(),
            clock.clone(),
        ),
        PublishSlot::new(slots.clone(), analytics.clone(), clock.clone()),
        SearchSlots::new(slots, clock.clone()),
        CreateBooking::new(bookings.clone(), analytics.clone(), clock.clone()),
        ConfirmBooking::new(
            bookings.clone(),
            EntitlementQuotaGate::new(entitlements.clone()),
            analytics.clone(),
            events.clone(),
            clock.clone(),
        ),
        CancelBooking::new(bookings, analytics.clone(), events.clone(), clock.clone()),
        StartSubscription::new(entitlements.clone(), analytics, events, clock.clone()),
        GetEntitlement::new(entitlements, clock),
        MockTokenService,
    )
}
