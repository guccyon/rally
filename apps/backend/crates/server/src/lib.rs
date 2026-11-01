//! 組み立て用のクレート（composition root）。interface のルーターに、ポートの実装を渡す。
//!
//! interface と infra の両方に依存してよいのはこのクレートだけ（docs/adr/0003-backend-layers.md の決定 5）。

// 配信先の選択は seed と共有するので infra に置き、ここでは同じ名前で公開し続ける
pub use infra::AnalyticsDestination;
// 通話基盤の選択は配信先の選択と同じく infra に置き、ここでも公開する
pub use infra::{UnknownVideoCallProvider, VideoCallBackend};

mod assembly;
mod features;
mod ports;

use application::{
    analytics::AnalyticsSink,
    async_lesson::VideoRepository,
    catalog::LessonMenuRepository,
    clock::{Clock, SystemClock},
    entitlement::{EntitlementRepository, LiveLessonQuotaSubscriber},
    identity::AccountRepository,
    in_memory::{
        InMemoryAccountRepository, InMemoryAnnouncementDraftRepository,
        InMemoryAnnouncementRepository, InMemoryBookingRepository,
        InMemoryCoachAcceptanceRepository, InMemoryEntitlementRepository,
        InMemoryFavoriteCoachRepository, InMemoryLessonCompletionRepository,
        InMemoryLessonMenuRepository, InMemoryLessonReviewRepository,
        InMemoryLiveLessonSessionRepository, InMemoryMessageThreadRepository,
        InMemoryNotificationRepository, InMemoryNotificationSettingsRepository,
        InMemorySlotRepository, InMemoryVideoRepository, RecordingAnalyticsSink,
    },
    lesson::LessonCompletionRepository,
    live_lesson::{LiveLessonSessionRepository, VideoCallProvider},
    matching::FavoriteCoachRepository,
    messaging::{MessageThreadRepository, MessageThreadSubscriber},
    notification::{
        AnnouncementDraftRepository, AnnouncementRepository, NotificationRepository,
        NotificationSettingsRepository, PushNotificationSubscriber, PushSender,
    },
    profile::CoachProfileRepository,
    review::LessonReviewRepository,
    scheduling::{BookingRepository, CoachAcceptanceRepository, SlotRepository},
};
use axum::Router;
use infra::{
    InMemoryDomainEventBus, MockPushSender, PgAccountRepository, PgAnnouncementDraftRepository,
    PgAnnouncementRepository, PgBookingRepository, PgCoachAcceptanceRepository,
    PgEntitlementRepository, PgFavoriteCoachRepository, PgLessonCompletionRepository,
    PgLessonMenuRepository, PgLessonReviewRepository, PgLiveLessonSessionRepository,
    PgMessageThreadRepository, PgNotificationRepository, PgNotificationSettingsRepository,
    PgSlotRepository, PgVideoRepository,
};
use sqlx::PgPool;

use crate::assembly::Assembly;

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
/// （`GET` / `PUT /notification-settings/me`）でオフの種類は送らない。送る通知はオフの種類も含めてお知らせとして残し、
/// 生徒とコーチは自分のお知らせを一覧で読んで既読にできる（`GET /notifications/me`・`POST /notifications/me/read`）。受け取り設定と
/// お知らせの保存先も当面インメモリ。本部は生徒全員か、あるスポーツの生徒全員にお知らせを送り（`POST /announcements`）、
/// 送った記録を一覧で見る（`GET /announcements`）。受け取る生徒はアカウントの保存先から読み
/// （`ProfileAnnouncementRecipientLookup`）、生徒ごとにお知らせを残して、受け取り設定に従って同じ送り先にプッシュ通知を送る。
/// 送った記録の保存先も当面インメモリ。本部は送る前のお知らせを下書きとして残し（`/announcement-drafts`。保存先は当面
/// インメモリ）、下書きから送る（送るのは `POST /announcements` と同じ組み立てで、送ったら下書きを消す）。
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
/// 本部が見る予約の一覧（`GET /bookings`）も同じく予約の保存先から期間に開始する予約を読み、コーチと生徒の表示名を
/// アカウントの保存先から読んで添える（`ListBookings`。`ProfileCoachDisplayNameLookup`・`ProfileStudentDisplayNameLookup`）。
/// 本部が見るキャンセルされた予約の一覧（`GET /bookings/cancelled`）は予約の保存先から期間にキャンセルした予約を読み、
/// 表示名を同じように添える（`ListCancelledBookings`）。
/// 本部が見る生徒の一覧（`GET /students`）はアカウントの保存先から生徒を読み、受講権の契約種別と契約プランを受講権の
/// 保存先から（`EntitlementStudentContractLookup`）、最後に予約した時刻を予約の保存先から（`SchedulingStudentLastBookingLookup`）
/// 読んで添える（identity は entitlement・scheduling の表を直接読まない）。
/// 本部が見る生徒 1 人の利用の様子（`GET /students/{student_id}/activity`）はアカウントの保存先から生徒の表示名を読み、
/// これからの予約とキャンセルの件数を予約の保存先から（`SchedulingStudentBookingActivityLookup`）、直近の受けたレッスンを
/// 完了記録の保存先から（`LessonStudentLessonHistoryLookup`）、コーチの表示名をアカウントの保存先から
/// （`ProfileCoachDisplayNameLookup`）読んで添える。
///
/// スロットの検索（`GET /slots`）と自分の予約の一覧（`GET /bookings/me`）は、出るコーチの表示名と自己紹介文をアカウントの保存先から読んで添える
/// （`ProfileCoachIntroductionLookup`。scheduling は profile を直接読まない）。コーチの自分の予約の一覧（`GET /coaches/me/bookings`）は
/// 予約した生徒の表示名を同じくアカウントの保存先から読んで添える（`ProfileStudentDisplayNameLookup`）。コーチに届いた動画の一覧
/// （`GET /coaches/me/videos`）も、動画を送った生徒の表示名を同じ実装で添える。生徒が送った動画の一覧（`GET /videos/me`）は、
/// 送り先のコーチの表示名をアカウントの保存先から読んで添える（`ProfileCoachDisplayNameLookup`）。本部が見るレッスンメニューの横断の一覧（`GET /lesson-menus`）も、作ったコーチの表示名を同じ実装で添える。コーチの自分の枠の一覧（`GET /coaches/me/slots`）は、
/// スロットの保存先から枠を、予約の保存先から枠に入った予約を読む（`ListCoachSlots`）。
/// 生徒が見るコーチの紹介（`GET /coaches/{coach_id}`）は、アカウントの保存先から表示名・自己紹介文・提供スポーツを読み、
/// お気に入りの保存先からその生徒のお気に入りかを読む（`GetCoachProfile`。`MatchingCoachFavoriteLookup`）。
/// 生徒のお気に入りのコーチ（`GET /favorite-coaches`・`PUT`・`DELETE /favorite-coaches/{coach_id}`）は、お気に入りの保存先
/// （当面インメモリ）に入れ・外し、一覧はコーチの表示名と提供スポーツをアカウントの保存先から、評価の件数と平均を評価の
/// 保存先から読んで添える（`ListFavoriteCoaches`）。コーチを探す一覧（`GET /coaches`）にも、お気に入りかを添える。
///
/// レッスンの評価（`POST /lesson-reviews`）は完了記録の保存先から評価するレッスンを読み（`LessonCompletionLookup`）、
/// 評価の保存先も当面インメモリ。コーチの評価の件数と平均（`GET /coaches/{coach_id}/review-summary`）と
/// 評価の一覧（`GET /coaches/{coach_id}/reviews`）は同じ保存先から読む。評価の一覧に添えるレッスンメニューは、
/// 完了記録の元（通話セッションに結び付いた予約・動画）が持つ写しを読む（`LessonCompletionMenuLookup`。予約は
/// `SchedulingBookingLookup` で読み、review は予約・動画を直接読まない）。本部が見る低い評価の一覧
/// （`GET /lesson-reviews/low`）も同じ保存先から読み、同じメニューの写しと、コーチ・生徒の表示名をアカウントの保存先から
/// （`ProfileCoachDisplayNameLookup`・`ProfileStudentDisplayNameLookup`）読んで添える。スポーツで絞るときは、評価した
/// レッスンのスポーツを完了記録の元（予約の枠・動画）から読む（`LessonCompletionSportLookup`）。
///
/// 生徒が受けたレッスンの一覧（`GET /lessons/completions/me`）は完了記録の保存先から生徒の完了したレッスンを読み、
/// メニューは評価の一覧と同じく完了記録の元が持つ写しを、コーチの表示名は `ProfileCoachDisplayNameLookup` で、評価を
/// 付けたかは評価の保存先から（`ReviewLessonCompletionReviewLookup`）読む（完了記録は profile・review を直接読まない）。
///
/// 本部によるコーチの受付の停止・再開（`POST /coaches/{coach_id}/suspend`・`/resume`）は、コーチがいるかをアカウントの
/// 保存先に問い合わせ（`ProfileCoachIntroductionLookup`）、コーチの受付の保存先に書く（`SuspendCoach`・`ResumeCoach`）。
/// 受付の状態はスロットの検索と予約の作成が同じ中身を読み、コーチの一覧（`GET /coaches`）は scheduling に問い合わせて
/// 添える・除く（`SchedulingCoachAcceptanceLookup`）。インメモリの組み立てではスロットの保存先と中身を共有する。
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
    P: PushSender + Clone + 'static,
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
    InMemoryNotificationRepository,
    InMemoryLessonMenuRepository,
    InMemoryCoachAcceptanceRepository,
    InMemoryFavoriteCoachRepository,
    InMemoryAnnouncementRepository,
    InMemoryAnnouncementDraftRepository,
> {
    let slots = InMemorySlotRepository::default();
    Repositories {
        coach_acceptances: slots.coach_acceptances(),
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
        notifications: InMemoryNotificationRepository::default(),
        lesson_menus: InMemoryLessonMenuRepository::default(),
        favorite_coaches: InMemoryFavoriteCoachRepository::default(),
        announcements: InMemoryAnnouncementRepository::default(),
        announcement_drafts: InMemoryAnnouncementDraftRepository::default(),
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
            notifications: PgNotificationRepository::new(pool.clone()),
            coach_acceptances: PgCoachAcceptanceRepository::new(pool.clone()),
            lesson_menus: PgLessonMenuRepository::new(pool.clone()),
            favorite_coaches: PgFavoriteCoachRepository::new(pool.clone()),
            announcements: PgAnnouncementRepository::new(pool.clone()),
            announcement_drafts: PgAnnouncementDraftRepository::new(pool),
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

/// 集約ごとの保存先。予約の保存先はスロットの保存先と同じ中身を読み書きする。コーチの受付の保存先も、スロットの検索と
/// 予約の作成が読むのと同じ中身を読み書きする。
struct Repositories<R, S, B, E, L, W, M, X, N, Z, K, J, Q, G, H, F> {
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
    notifications: K,
    lesson_menus: J,
    coach_acceptances: Q,
    favorite_coaches: G,
    announcements: H,
    announcement_drafts: F,
}

/// 保存先のほかの組み立て（インメモリと Postgres で共通）。境界間イベントのバスを作り、機能ごとの組み立て（[`features`]）に
/// ポートの実装をまとめて渡す。
#[allow(clippy::type_complexity)]
fn compose<R, S, B, E, L, W, M, X, N, Z, K, J, Q, G, H, F, C, A, P, V>(
    repositories: Repositories<R, S, B, E, L, W, M, X, N, Z, K, J, Q, G, H, F>,
    clock: C,
    analytics: A,
    push: P,
    video: V,
) -> Router
where
    R: AccountRepository + CoachProfileRepository + Clone + 'static,
    S: SlotRepository + Clone + 'static,
    B: BookingRepository + Clone + 'static,
    E: EntitlementRepository + Clone + 'static,
    L: LiveLessonSessionRepository + Clone + 'static,
    W: VideoRepository + Clone + 'static,
    M: LessonCompletionRepository + Clone + 'static,
    X: MessageThreadRepository + Clone + 'static,
    N: LessonReviewRepository + Clone + 'static,
    Z: NotificationSettingsRepository + Clone + 'static,
    K: NotificationRepository + Clone + 'static,
    J: LessonMenuRepository + Clone + 'static,
    Q: CoachAcceptanceRepository + Clone + 'static,
    G: FavoriteCoachRepository + Clone + 'static,
    H: AnnouncementRepository + Clone + 'static,
    F: AnnouncementDraftRepository + Clone + 'static,
    C: Clock + Clone + 'static,
    A: AnalyticsSink + Clone + 'static,
    P: PushSender + Clone + 'static,
    V: VideoCallProvider + Clone + 'static,
{
    let events = InMemoryDomainEventBus::new((
        LiveLessonQuotaSubscriber::new(repositories.entitlements.clone()),
        (
            MessageThreadSubscriber::new(repositories.threads.clone()),
            PushNotificationSubscriber::new(
                push.clone(),
                repositories.notification_settings.clone(),
                repositories.notifications.clone(),
                clock.clone(),
            ),
        ),
    ));
    features::router(&Assembly {
        repositories,
        clock,
        analytics,
        video_call: video,
        push,
        events,
    })
}
