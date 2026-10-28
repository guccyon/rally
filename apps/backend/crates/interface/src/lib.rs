//! インターフェース層。axum の HTTP ルーティングを置く。
//!
//! 依存してよいのは application と domain。infra には直接依存しない
//! （docs/adr/0003-backend-layers.md）。ポートの実装は組み立て用のクレート（`server`）が渡す。

mod accounts;
pub mod auth;
mod bookings;
mod coaches;
mod contract;
mod entitlements;
mod error;
mod lessons;
mod live_lesson_sessions;
mod me;
mod message;
mod messaging;
mod notification_settings;
mod reviews;
mod slot_supply;
mod slots;
mod subscriptions;
mod videos;

use std::sync::Arc;

use application::{
    analytics::AnalyticsSink,
    async_lesson::{
        GetVideoReview, ReturnVideoReview, ReviewGate, SubmitVideo, VideoRepository, VideoStorage,
    },
    clock::Clock,
    domain_event::DomainEventPublisher,
    entitlement::{EntitlementRepository, GetEntitlement, StartSubscription},
    identity::{
        AccountRepository, GetAccount, ListCoaches, RegisterAccount, TokenIssuer, TokenVerifier,
        UpdateCoachProfile,
    },
    lesson::{LessonCompletionRepository, RecordLessonCompletion},
    live_lesson::{
        BookingLookup, EndLiveLessonSession, LiveLessonSessionRepository, StartLiveLessonSession,
        VideoCallProvider,
    },
    messaging::{
        ChatGate, ListMessageThreads, ListMessages, MarkMessageThreadRead, MessageThreadRepository,
        SendMessage,
    },
    notification::{
        GetNotificationSettings, NotificationSettingsRepository, UpdateNotificationSettings,
    },
    review::{
        CompletedLessonLookup, GetCoachReviewSummary, LessonReviewRepository, ListCoachReviews,
        SubmitLessonReview,
    },
    scheduling::{
        BookingRepository, CancelBooking, CoachIntroductionLookup, ConfirmBooking, CreateBooking,
        GetSlotSupply, PublishSlot, QuotaGate, SearchSlots, SlotRepository,
    },
};
use axum::{
    Router,
    http::StatusCode,
    routing::{get, post, put},
};
use domain::identity::Role;

pub use auth::{AuthenticatedAccount, require_role};
pub use error::ApiError;

/// ユースケースとアクセストークンのポート（発行・検証）を外から渡して組み立てるルーター。
///
/// `/healthz` と `/accounts` 以外のルートは認証必須（[`AuthenticatedAccount`] を取る）。
/// `POST /slots`（admin-web）はコーチだけ、`GET /slots`（student-app）は認証した利用者なら誰でも呼べる。
/// `/bookings`（student-app の予約の作成・確定・キャンセル）・`POST /subscriptions`（月額契約の開始）・
/// `GET /entitlements/me`（受講権の照会）は生徒だけ。`/live-lesson-sessions`（通話セッションの開始・終了）は
/// 役割で絞らず、予約の参加者（生徒・コーチ）かをユースケースが確かめる（契約に 403 がない）。
/// `POST /videos`（動画の送付）は生徒だけ、`POST /videos/{video_id}/review`（admin-web の添削の返却）はコーチだけ。
/// `GET /videos/{video_id}/review`（添削結果の取得）は役割で絞らず、動画の当事者かをユースケースが確かめる。
/// `POST /lessons/completions`（レッスンの完了記録）も役割で絞らず、通話セッション・動画の当事者かをユースケースが
/// 確かめる（契約に 403 がない）。`GET /slot-supply`（admin-web のスポーツごとの予約枠の需給）は本部だけ。
/// `/message-threads`（student-app・admin-web の生徒と担当コーチのチャット）は役割で絞らず、スレッドの当事者かを
/// ユースケースが確かめる（生徒がチャットの送り放題を持たない送信だけが 403）。
/// `POST /lesson-reviews`（レッスンの評価）は生徒だけで、評価できるのは自分が生徒の完了したレッスンだけ。
/// `GET /coaches/{coach_id}/review-summary`（admin-web のコーチの評価の件数と平均）と `GET /coaches/{coach_id}/reviews`
/// （コーチが受けた評価の一覧）は本部とコーチで、コーチは自分の分だけ。`GET /coaches`（admin-web のコーチの一覧）は本部だけ。
/// `GET /me`（admin-web のログイン中の本人のアカウント）は役割で絞らない。`PUT /me/profile`（admin-web の自分の
/// 自己紹介文の変更）はコーチだけ。
/// `/notification-settings/me`（student-app の自分の通知の受け取り設定の照会・変更）は生徒だけ。
#[allow(clippy::too_many_arguments)]
pub fn router_with<R, S, I, B, G, E, L, K, V, W, U, Q, M, X, H, O, N, Z, A, P, C, T>(
    register_account: RegisterAccount<R, A, C>,
    list_coaches: ListCoaches<R>,
    get_account: GetAccount<R>,
    update_coach_profile: UpdateCoachProfile<R>,
    publish_slot: PublishSlot<S, A, C>,
    search_slots: SearchSlots<S, I, C>,
    create_booking: CreateBooking<B, A, C>,
    confirm_booking: ConfirmBooking<B, G, A, P, C>,
    cancel_booking: CancelBooking<B, A, P, C>,
    get_slot_supply: GetSlotSupply<B, C>,
    start_subscription: StartSubscription<E, A, P, C>,
    get_entitlement: GetEntitlement<E, C>,
    start_live_lesson_session: StartLiveLessonSession<L, K, V, A, C>,
    end_live_lesson_session: EndLiveLessonSession<L, V, A, C>,
    submit_video: SubmitVideo<W, U, A, C>,
    get_video_review: GetVideoReview<W>,
    return_video_review: ReturnVideoReview<W, Q, A, P, C>,
    record_lesson_completion: RecordLessonCompletion<L, W, M, A, P, C>,
    list_message_threads: ListMessageThreads<X>,
    list_messages: ListMessages<X>,
    send_message: SendMessage<X, H, A, P, C>,
    mark_message_thread_read: MarkMessageThreadRead<X, C>,
    submit_lesson_review: SubmitLessonReview<O, N, A, C>,
    get_coach_review_summary: GetCoachReviewSummary<N>,
    list_coach_reviews: ListCoachReviews<N>,
    get_notification_settings: GetNotificationSettings<Z>,
    update_notification_settings: UpdateNotificationSettings<Z>,
    tokens: T,
) -> Router
where
    R: AccountRepository + 'static,
    S: SlotRepository + 'static,
    I: CoachIntroductionLookup + 'static,
    B: BookingRepository + 'static,
    G: QuotaGate + 'static,
    E: EntitlementRepository + 'static,
    L: LiveLessonSessionRepository + 'static,
    K: BookingLookup + 'static,
    V: VideoCallProvider + 'static,
    W: VideoRepository + 'static,
    U: VideoStorage + 'static,
    Q: ReviewGate + 'static,
    M: LessonCompletionRepository + 'static,
    X: MessageThreadRepository + 'static,
    H: ChatGate + 'static,
    O: CompletedLessonLookup + 'static,
    N: LessonReviewRepository + 'static,
    Z: NotificationSettingsRepository + 'static,
    A: AnalyticsSink + 'static,
    P: DomainEventPublisher + 'static,
    C: Clock + 'static,
    T: TokenIssuer + TokenVerifier + 'static,
{
    let tokens = Arc::new(tokens);
    let accounts = Router::new()
        .route("/accounts", post(accounts::register_account::<R, A, C>))
        .with_state(Arc::new(accounts::AccountsState {
            register_account,
            tokens: tokens.clone(),
        }));
    let coaches = Router::new()
        .route("/coaches", get(coaches::list_coaches::<R>))
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(coaches::CoachesState { list_coaches }));
    let me = Router::new()
        .route("/me", get(me::get_my_account::<R>))
        .route(
            "/me/profile",
            put(me::update_my_coach_profile::<R>).route_layer(require_role(&[Role::Coach])),
        )
        .with_state(Arc::new(me::MeState {
            get_account,
            update_coach_profile,
        }));
    let slots = Router::new()
        .route(
            "/slots",
            get(slots::search_slots::<S, I, A, C>).merge(
                post(slots::publish_slot::<S, I, A, C>).route_layer(require_role(&[Role::Coach])),
            ),
        )
        .with_state(Arc::new(slots::SlotsState {
            publish_slot,
            search_slots,
        }));
    let bookings = Router::new()
        .route("/bookings", post(bookings::create_booking::<B, G, A, P, C>))
        .route(
            "/bookings/{booking_id}/confirm",
            post(bookings::confirm_booking::<B, G, A, P, C>),
        )
        .route(
            "/bookings/{booking_id}/cancel",
            post(bookings::cancel_booking::<B, G, A, P, C>),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(bookings::BookingsState {
            create_booking,
            confirm_booking,
            cancel_booking,
        }));
    let slot_supply = Router::new()
        .route("/slot-supply", get(slot_supply::get_slot_supply::<B, C>))
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(slot_supply::SlotSupplyState { get_slot_supply }));
    let entitlements = Router::new()
        .route(
            "/entitlements/me",
            get(entitlements::get_my_entitlement::<E, C>),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(entitlements::EntitlementsState {
            get_entitlement,
        }));
    let subscriptions = Router::new()
        .route(
            "/subscriptions",
            post(subscriptions::start_subscription::<E, A, P, C>),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(subscriptions::SubscriptionsState {
            start_subscription,
        }));
    let live_lesson_sessions = Router::new()
        .route(
            "/live-lesson-sessions",
            post(live_lesson_sessions::start_live_lesson_session::<L, K, V, A, C>),
        )
        .route(
            "/live-lesson-sessions/{session_id}/end",
            post(live_lesson_sessions::end_live_lesson_session::<L, K, V, A, C>),
        )
        .with_state(Arc::new(live_lesson_sessions::LiveLessonSessionsState {
            start_live_lesson_session,
            end_live_lesson_session,
        }));
    let videos = Router::new()
        .route(
            "/videos",
            post(videos::submit_video::<W, U, Q, A, P, C>)
                .route_layer(require_role(&[Role::Student])),
        )
        .route(
            "/videos/{video_id}/review",
            get(videos::get_video_review::<W, U, Q, A, P, C>).merge(
                post(videos::return_video_review::<W, U, Q, A, P, C>)
                    .route_layer(require_role(&[Role::Coach])),
            ),
        )
        .with_state(Arc::new(videos::VideosState {
            submit_video,
            get_video_review,
            return_video_review,
        }));
    let lessons = Router::new()
        .route(
            "/lessons/completions",
            post(lessons::record_lesson_completion::<L, W, M, A, P, C>),
        )
        .with_state(Arc::new(lessons::LessonsState {
            record_lesson_completion,
        }));
    let messaging = Router::new()
        .route(
            "/message-threads",
            get(messaging::list_message_threads::<X, H, A, P, C>),
        )
        .route(
            "/message-threads/{thread_id}/messages",
            get(messaging::list_messages::<X, H, A, P, C>)
                .post(messaging::send_message::<X, H, A, P, C>),
        )
        .route(
            "/message-threads/{thread_id}/read",
            post(messaging::mark_message_thread_read::<X, H, A, P, C>),
        )
        .with_state(Arc::new(messaging::MessagingState {
            list_message_threads,
            list_messages,
            send_message,
            mark_message_thread_read,
        }));
    let reviews = Router::new()
        .route(
            "/lesson-reviews",
            post(reviews::submit_lesson_review::<O, N, A, C>)
                .route_layer(require_role(&[Role::Student])),
        )
        .route(
            "/coaches/{coach_id}/review-summary",
            get(reviews::get_coach_review_summary::<O, N, A, C>)
                .route_layer(require_role(&[Role::Headquarters, Role::Coach])),
        )
        .route(
            "/coaches/{coach_id}/reviews",
            get(reviews::list_coach_reviews::<O, N, A, C>)
                .route_layer(require_role(&[Role::Headquarters, Role::Coach])),
        )
        .with_state(Arc::new(reviews::ReviewsState {
            submit_lesson_review,
            get_coach_review_summary,
            list_coach_reviews,
        }));
    let notification_settings = Router::new()
        .route(
            "/notification-settings/me",
            get(notification_settings::get_my_notification_settings::<Z>)
                .put(notification_settings::update_my_notification_settings::<Z>),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(notification_settings::NotificationSettingsState {
            get_notification_settings,
            update_notification_settings,
        }));
    Router::new()
        .route("/healthz", get(healthz))
        .merge(accounts)
        .merge(coaches)
        .merge(me)
        .merge(slots)
        .merge(bookings)
        .merge(slot_supply)
        .merge(subscriptions)
        .merge(entitlements)
        .merge(live_lesson_sessions)
        .merge(videos)
        .merge(lessons)
        .merge(messaging)
        .merge(reviews)
        .merge(notification_settings)
        .layer(auth::TokenVerification::layer(tokens))
}

/// 死活監視。プロセスが要求を受け付けられれば 200 を返す。
async fn healthz() -> (StatusCode, &'static str) {
    (StatusCode::OK, "ok")
}
