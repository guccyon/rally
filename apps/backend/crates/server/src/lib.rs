//! 組み立て用のクレート（composition root）。interface のルーターに、ポートの実装を渡す。
//!
//! interface と infra の両方に依存してよいのはこのクレートだけ（docs/adr/0003-backend-layers.md の決定 5）。

// 配信先の選択は seed と共有するので infra に置き、ここでは同じ名前で公開し続ける
pub use infra::AnalyticsDestination;
// 通話基盤の選択は配信先の選択と同じく infra に置き、ここでも公開する
pub use infra::{UnknownVideoCallProvider, VideoCallBackend};

use application::{
    analytics::AnalyticsSink,
    async_lesson::{
        GetVideoReview, ListCoachVideos, ReturnVideoReview, SubmitVideo, VideoRepository,
    },
    catalog::{
        CatalogLessonMenuLookup, CreateLessonMenu, LessonMenuRepository, ListCoachLessonMenus,
        ListMyLessonMenus, UpdateLessonMenu,
    },
    clock::{Clock, SystemClock},
    entitlement::{
        EntitlementChatGate, EntitlementQuotaGate, EntitlementRepository, EntitlementReviewGate,
        GetEntitlement, LiveLessonQuotaSubscriber, StartSubscription,
    },
    identity::{AccountRepository, GetAccount, RegisterAccount},
    in_memory::{
        InMemoryAccountRepository, InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemoryLessonCompletionRepository, InMemoryLessonMenuRepository,
        InMemoryLessonReviewRepository, InMemoryLiveLessonSessionRepository,
        InMemoryMessageThreadRepository, InMemoryNotificationSettingsRepository,
        InMemorySlotRepository, InMemoryVideoRepository, RecordingAnalyticsSink,
    },
    lesson::{
        LessonCompletionLookup, LessonCompletionMenuLookup, LessonCompletionRepository,
        RecordLessonCompletion,
    },
    live_lesson::{
        EndLiveLessonSession, LiveLessonSessionRepository, StartLiveLessonSession,
        VideoCallProvider,
    },
    messaging::{
        ListMessageThreads, ListMessages, MarkMessageThreadRead, MessageThreadRepository,
        MessageThreadSubscriber, SendMessage,
    },
    notification::{
        GetNotificationSettings, NotificationSettingsRepository, PushNotificationSubscriber,
        PushSender, UpdateNotificationSettings,
    },
    profile::{
        ListCoaches, ProfileCoachIntroductionLookup, ProfileStudentDisplayNameLookup,
        UpdateCoachProfile,
    },
    review::{GetCoachReviewSummary, LessonReviewRepository, ListCoachReviews, SubmitLessonReview},
    scheduling::{
        BookingRepository, CancelBooking, ConfirmBooking, CreateBooking, GetSlotSupply,
        ListCoachBookings, ListCoachSlots, ListMyBookings, PublishSlot, SchedulingBookingLookup,
        SearchSlots, SlotRepository,
    },
};
use axum::Router;
use infra::{
    InMemoryDomainEventBus, MockAiFeedbackGenerator, MockPushSender, MockTokenService,
    MockVideoStorage, PgAccountRepository, PgBookingRepository, PgEntitlementRepository,
    PgLessonCompletionRepository, PgLessonMenuRepository, PgLessonReviewRepository,
    PgLiveLessonSessionRepository, PgMessageThreadRepository, PgNotificationSettingsRepository,
    PgSlotRepository, PgVideoRepository,
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
/// 予約の確定で、その生徒とコーチのチャットのスレッドが開く。動画の送付（`VideoReviewRequested`）と添削の返却
/// （`VideoReviewReturned`）では、そのスレッドに（なければ開いて）添削依頼・添削の返却のメッセージが足される。
/// 予約の確定で、生徒にプッシュ通知を送る（送り先の基盤は infra のモック `MockPushSender`）。受け取る人の通知の受け取り設定
/// （`GET` / `PUT /notification-settings/me`）でオフの種類は送らない。受け取り設定の保存先も当面インメモリ。
///
/// チャットの送信は entitlement にその生徒がチャットを送ってよいか（月額は送り放題）を問い合わせて
/// （`EntitlementChatGate`。照会だけで回数を消費しない）組み立てる。スレッドの保存先も当面インメモリ。
///
/// 通話セッションの開始・終了は通話基盤のモック（既定の [`VideoCallBackend`]。Twilio Video 相当の `MockVideoCallProvider`。
/// 終了時に接続品質の指標を乱数で生成する）で組み立て、開始は scheduling の予約の保存先に予約を問い合わせる（`SchedulingBookingLookup`）。
///
/// 動画の送付は動画の保存先のモック（`MockVideoStorage`。固定の形のアップロード URL を返す）と AI の一次フィードバックの
/// モック（`MockAiFeedbackGenerator`。スポーツごとの決まった文を返す）で、添削の返却は
/// entitlement に着手してよいかを問い合わせて（`EntitlementReviewGate`。月額は常に通し、ゴルフは通さない）組み立てる。
/// 動画と添削の保存先は当面インメモリ。
///
/// レッスンの完了記録は、通話セッション・動画の保存先と同じ中身を読み（元の集約が完了記録を発行する。
/// docs/adr/0006）、完了記録の保存先も当面インメモリ。境界間イベント `LessonCompleted` は同じイベントバスで発行する
/// （billing・review の購読はまだない）。
///
/// スポーツごとの予約枠の需給（本部の `GET /slot-supply`）は予約の保存先から期間の枠と予約を読む（`GetSlotSupply`）。
///
/// スロットの検索（`GET /slots`）と自分の予約の一覧（`GET /bookings/me`）は、出るコーチの表示名と自己紹介文をアカウントの保存先から読んで添える
/// （`ProfileCoachIntroductionLookup`。scheduling は profile を直接読まない）。コーチの自分の予約の一覧（`GET /coaches/me/bookings`）は
/// 予約した生徒の表示名を同じくアカウントの保存先から読んで添える（`ProfileStudentDisplayNameLookup`）。コーチに届いた動画の一覧
/// （`GET /coaches/me/videos`）も、動画を送った生徒の表示名を同じ実装で添える。コーチの自分の枠の一覧（`GET /coaches/me/slots`）は、
/// スロットの保存先から枠を、予約の保存先から枠に入った予約を読む（`ListCoachSlots`）。
///
/// レッスンの評価（`POST /lesson-reviews`）は完了記録の保存先から評価するレッスンを読み（`LessonCompletionLookup`）、
/// 評価の保存先も当面インメモリ。コーチの評価の件数と平均（`GET /coaches/{coach_id}/review-summary`）と
/// 評価の一覧（`GET /coaches/{coach_id}/reviews`）は同じ保存先から読む。評価の一覧に添えるレッスンメニューは、
/// 完了記録の元（通話セッションに結び付いた予約・動画）が持つ写しを読む（`LessonCompletionMenuLookup`。予約は
/// `SchedulingBookingLookup` で読み、review は予約・動画を直接読まない）。
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
    router_with_video_call(clock, analytics, VideoCallBackend::default())
}

/// [`router_with_analytics`] の通話基盤を `video` に替えたルーター（`main` は [`VideoCallBackend`] で選ぶ）。
pub fn router_with_video_call<C, A, V>(clock: C, analytics: A, video: V) -> Router
where
    C: Clock + Clone + 'static,
    A: AnalyticsSink + Clone + 'static,
    V: VideoCallProvider + Clone + 'static,
{
    compose(
        in_memory_repositories(),
        clock,
        analytics,
        MockPushSender::new(),
        video,
    )
}

/// [`router_with_analytics`] のプッシュ通知の送り先を `push` に替えたルーター（テストが送った通知を読むのに使う）。
pub fn router_with_push_sender<C, A, P>(clock: C, analytics: A, push: P) -> Router
where
    C: Clock + Clone + 'static,
    A: AnalyticsSink + Clone + 'static,
    P: PushSender + 'static,
{
    compose(
        in_memory_repositories(),
        clock,
        analytics,
        push,
        VideoCallBackend::default(),
    )
}

/// 保存先をすべてインメモリにした集約ごとの保存先。
#[allow(clippy::type_complexity)]
fn in_memory_repositories() -> Repositories<
    InMemoryAccountRepository,
    InMemorySlotRepository,
    InMemoryBookingRepository,
    InMemoryEntitlementRepository,
    InMemoryLiveLessonSessionRepository,
    InMemoryVideoRepository,
    InMemoryLessonCompletionRepository,
    InMemoryMessageThreadRepository,
    InMemoryLessonReviewRepository,
    InMemoryNotificationSettingsRepository,
    InMemoryLessonMenuRepository,
> {
    let slots = InMemorySlotRepository::default();
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
        notification_settings: InMemoryNotificationSettingsRepository::default(),
        lesson_menus: InMemoryLessonMenuRepository::default(),
    }
}

/// 保存先をすべて infra の Postgres の実装にしたルーター（`pool` はマイグレーション `infra::MIGRATOR` を当てた DB）。
/// 保存先のほかは [`router_with_analytics`] と同じで、境界間イベントはインメモリのイベントバス、通話基盤・動画の
/// 保存先・プッシュ通知の送り先・アクセストークンは infra のモック。一周シナリオの E2E（`tests/walking_skeleton.rs`）が使う。
pub fn router_with_postgres<C, A>(pool: PgPool, clock: C, analytics: A) -> Router
where
    C: Clock + Clone + 'static,
    A: AnalyticsSink + Clone + 'static,
{
    router_with_postgres_and_video_call(pool, clock, analytics, VideoCallBackend::default())
}

/// [`router_with_postgres`] の通話基盤を `video` に替えたルーター（`DATABASE_URL` があるときの `main` が
/// [`VideoCallBackend`] で選んで使う）。
pub fn router_with_postgres_and_video_call<C, A, V>(
    pool: PgPool,
    clock: C,
    analytics: A,
    video: V,
) -> Router
where
    C: Clock + Clone + 'static,
    A: AnalyticsSink + Clone + 'static,
    V: VideoCallProvider + Clone + 'static,
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
            reviews: PgLessonReviewRepository::new(pool.clone()),
            notification_settings: PgNotificationSettingsRepository::new(pool.clone()),
            lesson_menus: PgLessonMenuRepository::new(pool),
        },
        clock,
        analytics,
        MockPushSender::new(),
        video,
    )
}

/// 版を読む環境変数の名前（`GET /version` が返す）。
pub const APP_VERSION_VAR: &str = "APP_VERSION";

/// `router` に、版を答える `GET /version`（認証なし。契約に載せない運用の口）を足す。版は `var`（環境変数の名前から
/// 値）で [`APP_VERSION_VAR`] を読み、ないか空なら `"unknown"`。
pub fn with_app_version(router: Router, var: impl Fn(&str) -> Option<String>) -> Router {
    router.merge(interface::version::router(var(APP_VERSION_VAR).as_deref()))
}

/// 集約ごとの保存先。予約の保存先はスロットの保存先と同じ中身を読み書きする。
struct Repositories<R, S, B, E, L, W, M, X, N, Z, J> {
    accounts: R,
    slots: S,
    bookings: B,
    entitlements: E,
    sessions: L,
    videos: W,
    completions: M,
    threads: X,
    reviews: N,
    notification_settings: Z,
    lesson_menus: J,
}

/// 保存先のほかの組み立て（インメモリと Postgres で共通）。
fn compose<R, S, B, E, L, W, M, X, N, Z, J, C, A, P, V>(
    repositories: Repositories<R, S, B, E, L, W, M, X, N, Z, J>,
    clock: C,
    analytics: A,
    push: P,
    video: V,
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
    Z: NotificationSettingsRepository + Clone + 'static,
    J: LessonMenuRepository + Clone + 'static,
    C: Clock + Clone + 'static,
    A: AnalyticsSink + Clone + 'static,
    P: PushSender + 'static,
    V: VideoCallProvider + Clone + 'static,
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
        notification_settings,
        lesson_menus,
    } = repositories;
    let catalog = interface::catalog_router(
        CreateLessonMenu::new(lesson_menus.clone(), accounts.clone(), clock.clone()),
        UpdateLessonMenu::new(lesson_menus.clone(), accounts.clone()),
        ListMyLessonMenus::new(lesson_menus.clone()),
        ListCoachLessonMenus::new(lesson_menus.clone()),
        MockTokenService,
    );
    let events = InMemoryDomainEventBus::new((
        LiveLessonQuotaSubscriber::new(entitlements.clone()),
        (
            MessageThreadSubscriber::new(threads.clone()),
            PushNotificationSubscriber::new(push, notification_settings.clone()),
        ),
    ));
    interface::router_with(
        RegisterAccount::new(accounts.clone(), analytics.clone(), clock.clone()),
        ListCoaches::new(accounts.clone()),
        GetAccount::new(accounts.clone()),
        UpdateCoachProfile::new(accounts.clone()),
        PublishSlot::new(
            slots.clone(),
            CatalogLessonMenuLookup::new(lesson_menus.clone()),
            analytics.clone(),
            clock.clone(),
        ),
        SearchSlots::new(
            slots.clone(),
            CatalogLessonMenuLookup::new(lesson_menus.clone()),
            ProfileCoachIntroductionLookup::new(accounts.clone()),
            clock.clone(),
        ),
        CreateBooking::new(
            bookings.clone(),
            CatalogLessonMenuLookup::new(lesson_menus.clone()),
            analytics.clone(),
            clock.clone(),
        ),
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
        ListMyBookings::new(
            bookings.clone(),
            ProfileCoachIntroductionLookup::new(accounts.clone()),
        ),
        ListCoachBookings::new(
            bookings.clone(),
            ProfileStudentDisplayNameLookup::new(accounts.clone()),
        ),
        ListCoachSlots::new(
            slots,
            CatalogLessonMenuLookup::new(lesson_menus.clone()),
            bookings.clone(),
        ),
        StartSubscription::new(
            entitlements.clone(),
            analytics.clone(),
            events.clone(),
            clock.clone(),
        ),
        GetEntitlement::new(entitlements.clone(), clock.clone()),
        StartLiveLessonSession::new(
            sessions.clone(),
            SchedulingBookingLookup::new(bookings.clone()),
            video.clone(),
            analytics.clone(),
            clock.clone(),
        ),
        EndLiveLessonSession::new(sessions.clone(), video, analytics.clone(), clock.clone()),
        SubmitVideo::new(
            videos.clone(),
            CatalogLessonMenuLookup::new(lesson_menus.clone()),
            MockVideoStorage,
            MockAiFeedbackGenerator,
            analytics.clone(),
            events.clone(),
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
        ListCoachVideos::new(
            videos.clone(),
            ProfileStudentDisplayNameLookup::new(accounts),
        ),
        RecordLessonCompletion::new(
            sessions.clone(),
            videos.clone(),
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
            LessonCompletionLookup::new(completions.clone()),
            reviews.clone(),
            analytics,
            clock,
        ),
        GetCoachReviewSummary::new(reviews.clone()),
        ListCoachReviews::new(
            reviews,
            LessonCompletionMenuLookup::new(
                completions,
                sessions,
                SchedulingBookingLookup::new(bookings),
                videos,
            ),
        ),
        GetNotificationSettings::new(notification_settings.clone()),
        UpdateNotificationSettings::new(notification_settings),
        MockTokenService,
    )
    .merge(catalog)
}
