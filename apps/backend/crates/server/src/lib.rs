//! 組み立て用のクレート（composition root）。interface のルーターに、ポートの実装を渡す。
//!
//! interface と infra の両方に依存してよいのはこのクレートだけ（docs/adr/0003-backend-layers.md の決定 5）。

// 配信先の選択は seed と共有するので infra に置き、ここでは同じ名前で公開し続ける
pub use infra::AnalyticsDestination;

use application::{
    analytics::AnalyticsSink,
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo, VideoRepository},
    clock::{Clock, SystemClock},
    entitlement::{
        EntitlementChatGate, EntitlementQuotaGate, EntitlementRepository, EntitlementReviewGate,
        GetEntitlement, LiveLessonQuotaSubscriber, StartSubscription,
    },
    identity::{AccountRepository, ListCoaches, RegisterAccount},
    in_memory::{
        InMemoryAccountRepository, InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemoryLessonCompletionRepository, InMemoryLessonReviewRepository,
        InMemoryLiveLessonSessionRepository, InMemoryMessageThreadRepository,
        InMemorySlotRepository, InMemoryVideoRepository, RecordingAnalyticsSink,
    },
    lesson::{LessonCompletionLookup, LessonCompletionRepository, RecordLessonCompletion},
    live_lesson::{EndLiveLessonSession, LiveLessonSessionRepository, StartLiveLessonSession},
    messaging::{
        ListMessageThreads, ListMessages, MarkMessageThreadRead, MessageThreadRepository,
        MessageThreadSubscriber, SendMessage,
    },
    notification::{PushNotificationSubscriber, PushSender},
    review::{GetCoachReviewSummary, LessonReviewRepository, ListCoachReviews, SubmitLessonReview},
    scheduling::{
        BookingRepository, CancelBooking, ConfirmBooking, CreateBooking, GetSlotSupply,
        PublishSlot, SchedulingBookingLookup, SearchSlots, SlotRepository,
    },
};
use axum::Router;
use infra::{
    InMemoryDomainEventBus, MockPushSender, MockTokenService, MockVideoCallProvider,
    MockVideoStorage, PgAccountRepository, PgBookingRepository, PgEntitlementRepository,
    PgLessonCompletionRepository, PgLessonReviewRepository, PgLiveLessonSessionRepository,
    PgMessageThreadRepository, PgSlotRepository, PgVideoRepository,
};
use sqlx::PgPool;

/// HTTP のルーター（現在時刻は OS の時計）。組み立ては [`router_at`]。
pub fn router() -> Router {
    router_at(SystemClock)
}

/// 現在時刻を `clock` から読むルーター。アカウント・スロット・予約・受講権・通話セッションの保存先と
/// 行動トラッキングは当面インメモリ、アクセストークンは infra のモック（署名なし。本番では差し替える）で組み立てる。
///
/// 境界間イベントは infra のインメモリのイベントバス（モック。標準出力にも書く）で発行し、entitlement の購読
/// （`LiveLessonQuotaSubscriber`）と messaging の購読（`MessageThreadSubscriber`）、notification の購読（`PushNotificationSubscriber`）につなぐ。予約の確定・キャンセルで
/// ビデオ通話の暦月の枠が動き、予約の確定は entitlement に枠が残っているかを問い合わせる（`EntitlementQuotaGate`）。
/// 予約の確定で、その生徒とコーチのチャットのスレッドが開く。
/// 予約の確定で、生徒にプッシュ通知を送る（送り先の基盤は infra のモック `MockPushSender`）。
///
/// チャットの送信は entitlement にその生徒がチャットを送ってよいか（月額は送り放題）を問い合わせて
/// （`EntitlementChatGate`。照会だけで回数を消費しない）組み立てる。スレッドの保存先も当面インメモリ。
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
/// スポーツごとの予約枠の需給（本部の `GET /slot-supply`）は予約の保存先から期間の枠と予約を読む（`GetSlotSupply`）。
///
/// レッスンの評価（`POST /lesson-reviews`）は完了記録の保存先から評価するレッスンを読み（`LessonCompletionLookup`）、
/// 評価の保存先も当面インメモリ。コーチの評価の件数と平均（`GET /coaches/{coach_id}/review-summary`）と
/// 評価の一覧（`GET /coaches/{coach_id}/reviews`）は同じ保存先から読む。
///
/// 保存先をすべて Postgres にした組み立ては [`router_with_postgres`]。
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
    router_with_push_sender(clock, analytics, MockPushSender::new())
}

/// [`router_with_analytics`] のプッシュ通知の送り先を `push` に替えたルーター（テストが送った通知を読むのに使う）。
pub fn router_with_push_sender<C, A, P>(clock: C, analytics: A, push: P) -> Router
where
    C: Clock + Clone + 'static,
    A: AnalyticsSink + Clone + 'static,
    P: PushSender + 'static,
{
    let slots = InMemorySlotRepository::default();
    compose(
        Repositories {
            accounts: InMemoryAccountRepository::default(),
            bookings: InMemoryBookingRepository::new(slots.clone()),
            slots,
            entitlements: InMemoryEntitlementRepository::default(),
            sessions: InMemoryLiveLessonSessionRepository::default(),
            videos: InMemoryVideoRepository::default(),
            completions: InMemoryLessonCompletionRepository::default(),
            threads: InMemoryMessageThreadRepository::default(),
            reviews: InMemoryLessonReviewRepository::default(),
        },
        clock,
        analytics,
        push,
    )
}

/// 保存先をすべて infra の Postgres の実装にしたルーター（`pool` はマイグレーション `infra::MIGRATOR` を当てた DB）。
/// 保存先のほかは [`router_with_analytics`] と同じで、境界間イベントはインメモリのイベントバス、通話基盤・動画の
/// 保存先・プッシュ通知の送り先・アクセストークンは infra のモック。一周シナリオの E2E（`tests/walking_skeleton.rs`）が使う。
pub fn router_with_postgres<C, A>(pool: PgPool, clock: C, analytics: A) -> Router
where
    C: Clock + Clone + 'static,
    A: AnalyticsSink + Clone + 'static,
{
    compose(
        Repositories {
            accounts: PgAccountRepository::new(pool.clone()),
            slots: PgSlotRepository::new(pool.clone()),
            bookings: PgBookingRepository::new(pool.clone()),
            entitlements: PgEntitlementRepository::new(pool.clone()),
            sessions: PgLiveLessonSessionRepository::new(pool.clone()),
            videos: PgVideoRepository::new(pool.clone()),
            completions: PgLessonCompletionRepository::new(pool.clone()),
            threads: PgMessageThreadRepository::new(pool.clone()),
            reviews: PgLessonReviewRepository::new(pool),
        },
        clock,
        analytics,
        MockPushSender::new(),
    )
}

/// 集約ごとの保存先。予約の保存先はスロットの保存先と同じ中身を読み書きする。
struct Repositories<R, S, B, E, L, W, M, X, N> {
    accounts: R,
    slots: S,
    bookings: B,
    entitlements: E,
    sessions: L,
    videos: W,
    completions: M,
    threads: X,
    reviews: N,
}

/// 保存先のほかの組み立て（インメモリと Postgres で共通）。
fn compose<R, S, B, E, L, W, M, X, N, C, A, P>(
    repositories: Repositories<R, S, B, E, L, W, M, X, N>,
    clock: C,
    analytics: A,
    push: P,
) -> Router
where
    R: AccountRepository + Clone + 'static,
    S: SlotRepository + Clone + 'static,
    B: BookingRepository + Clone + 'static,
    E: EntitlementRepository + Clone + 'static,
    L: LiveLessonSessionRepository + Clone + 'static,
    W: VideoRepository + Clone + 'static,
    M: LessonCompletionRepository + Clone + 'static,
    X: MessageThreadRepository + Clone + 'static,
    N: LessonReviewRepository + Clone + 'static,
    C: Clock + Clone + 'static,
    A: AnalyticsSink + Clone + 'static,
    P: PushSender + 'static,
{
    let Repositories {
        accounts,
        slots,
        bookings,
        entitlements,
        sessions,
        videos,
        completions,
        threads,
        reviews,
    } = repositories;
    let events = InMemoryDomainEventBus::new((
        LiveLessonQuotaSubscriber::new(entitlements.clone()),
        (
            MessageThreadSubscriber::new(threads.clone()),
            PushNotificationSubscriber::new(push),
        ),
    ));
    let video = MockVideoCallProvider::new();
    interface::router_with(
        RegisterAccount::new(accounts.clone(), analytics.clone(), clock.clone()),
        ListCoaches::new(accounts),
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
        GetSlotSupply::new(bookings.clone(), clock.clone()),
        StartSubscription::new(
            entitlements.clone(),
            analytics.clone(),
            events.clone(),
            clock.clone(),
        ),
        GetEntitlement::new(entitlements.clone(), clock.clone()),
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
            events.clone(),
            clock.clone(),
        ),
        RecordLessonCompletion::new(
            sessions,
            videos,
            completions.clone(),
            analytics.clone(),
            events.clone(),
            clock.clone(),
        ),
        ListMessageThreads::new(threads.clone()),
        ListMessages::new(threads.clone()),
        SendMessage::new(
            threads.clone(),
            EntitlementChatGate::new(entitlements),
            analytics.clone(),
            events,
            clock.clone(),
        ),
        MarkMessageThreadRead::new(threads, clock.clone()),
        SubmitLessonReview::new(
            LessonCompletionLookup::new(completions),
            reviews.clone(),
            analytics,
            clock,
        ),
        GetCoachReviewSummary::new(reviews.clone()),
        ListCoachReviews::new(reviews),
        MockTokenService,
    )
}
