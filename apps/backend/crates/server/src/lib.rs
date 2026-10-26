//! 組み立て用のクレート（composition root）。interface のルーターに、ポートの実装を渡す。
//!
//! interface と infra の両方に依存してよいのはこのクレートだけ（docs/adr/0003-backend-layers.md の決定 5）。

mod analytics_destination;

pub use analytics_destination::AnalyticsDestination;

use application::{
    analytics::AnalyticsSink,
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo},
    clock::{Clock, SystemClock},
    entitlement::{
        EntitlementQuotaGate, EntitlementReviewGate, GetEntitlement, LiveLessonQuotaSubscriber,
        StartSubscription,
    },
    identity::RegisterAccount,
    in_memory::{
        InMemoryAccountRepository, InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemoryLessonCompletionRepository, InMemoryLiveLessonSessionRepository,
        InMemorySlotRepository, InMemoryVideoRepository, RecordingAnalyticsSink,
    },
    lesson::RecordLessonCompletion,
    live_lesson::{EndLiveLessonSession, StartLiveLessonSession},
    scheduling::{
        CancelBooking, ConfirmBooking, CreateBooking, PublishSlot, SchedulingBookingLookup,
        SearchSlots,
    },
};
use axum::Router;
use infra::{InMemoryDomainEventBus, MockTokenService, MockVideoCallProvider, MockVideoStorage};

/// HTTP のルーター（現在時刻は OS の時計）。組み立ては [`router_at`]。
pub fn router() -> Router {
    router_at(SystemClock)
}

/// 現在時刻を `clock` から読むルーター。アカウント・スロット・予約・受講権・通話セッションの保存先と
/// 行動トラッキングは当面インメモリ、アクセストークンは infra のモック（署名なし。本番では差し替える）で組み立てる。
///
/// 境界間イベントは infra のインメモリのイベントバス（モック。標準出力にも書く）で発行し、entitlement の購読
/// （`LiveLessonQuotaSubscriber`）につなぐ。予約の確定・キャンセルでビデオ通話の暦月の枠が動き、予約の確定は
/// entitlement に枠が残っているかを問い合わせる（`EntitlementQuotaGate`）。
///
/// 通話セッションの開始・終了は通話基盤のモック（`MockVideoCallProvider`。終了時に接続品質の指標を乱数で生成する）で
/// 組み立て、開始は scheduling の予約の保存先に予約を問い合わせる（`SchedulingBookingLookup`）。
///
/// 動画の送付は動画の保存先のモック（`MockVideoStorage`。固定の形のアップロード URL を返す）で、添削の返却は
/// entitlement に着手してよいかを問い合わせて（`EntitlementReviewGate`。月額は常に通し、ゴルフは通さない）組み立てる。
/// 動画と添削の保存先は当面インメモリ。
///
/// レッスンの完了記録は、通話セッション・動画の保存先と同じ中身を読み（元の集約が完了記録を発行する。
/// docs/adr/0006）、完了記録の保存先も当面インメモリ。境界間イベント `LessonCompleted` は同じイベントバスで発行する
/// （billing・review の購読はまだない）。
///
/// スロットの Postgres 実装（`infra::PgSlotRepository`）は `scheduling.slots` の外部キーが
/// `identity.coaches` を指すので、アカウントの保存先を Postgres にするときに一緒に差し替える（予約・受講権も同じ）。
pub fn router_at<C>(clock: C) -> Router
where
    C: Clock + Clone + 'static,
{
    router_with_analytics(clock, RecordingAnalyticsSink::default())
}

/// [`router_at`] の行動イベントの配信先を `analytics` に替えたルーター（`main` は [`AnalyticsDestination`] で選ぶ）。
pub fn router_with_analytics<C, A>(clock: C, analytics: A) -> Router
where
    C: Clock + Clone + 'static,
    A: AnalyticsSink + Clone + 'static,
{
    let slots = InMemorySlotRepository::default();
    let bookings = InMemoryBookingRepository::new(slots.clone());
    let entitlements = InMemoryEntitlementRepository::default();
    let events = InMemoryDomainEventBus::new(LiveLessonQuotaSubscriber::new(entitlements.clone()));
    let sessions = InMemoryLiveLessonSessionRepository::default();
    let video = MockVideoCallProvider::new();
    let videos = InMemoryVideoRepository::default();
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
        CancelBooking::new(
            bookings.clone(),
            analytics.clone(),
            events.clone(),
            clock.clone(),
        ),
        StartSubscription::new(
            entitlements.clone(),
            analytics.clone(),
            events.clone(),
            clock.clone(),
        ),
        GetEntitlement::new(entitlements, clock.clone()),
        StartLiveLessonSession::new(
            sessions.clone(),
            SchedulingBookingLookup::new(bookings),
            video.clone(),
            analytics.clone(),
            clock.clone(),
        ),
        EndLiveLessonSession::new(sessions.clone(), video, analytics.clone(), clock.clone()),
        SubmitVideo::new(
            videos.clone(),
            MockVideoStorage,
            analytics.clone(),
            clock.clone(),
        ),
        GetVideoReview::new(videos.clone()),
        ReturnVideoReview::new(
            videos.clone(),
            EntitlementReviewGate,
            analytics.clone(),
            clock.clone(),
        ),
        RecordLessonCompletion::new(
            sessions,
            videos,
            InMemoryLessonCompletionRepository::default(),
            analytics,
            events,
            clock,
        ),
        MockTokenService,
    )
}
