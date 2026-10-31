//! インターフェース層。axum の HTTP ルーティングを置く。
//!
//! 依存してよいのは application と domain。infra には直接依存しない
//! （docs/adr/0003-backend-layers.md）。ポートの実装は組み立て用のクレート（`server`）が渡す。

mod accounts;
mod announcements;
pub mod auth;
mod bookings;
mod coach_acceptances;
mod coach_bookings;
mod coach_profiles;
mod coach_slots;
mod coach_videos;
mod coaches;
mod contract;
mod entitlement_error;
mod entitlements;
mod error;
mod favorite_coaches;
mod headquarters_bookings;
mod lesson_menus;
mod lesson_stats;
mod lessons;
mod live_lesson_sessions;
mod low_rated_reviews;
mod me;
mod message;
mod messaging;
mod messaging_error;
mod my_bookings;
mod my_lesson_completions;
mod my_profile;
mod my_videos;
mod notification_settings;
mod notifications;
mod review_error;
mod review_replies;
mod reviews;
mod scheduling_error;
mod slot_supply;
mod slot_withdrawals;
mod slots;
mod students;
mod subscription_plans;
mod subscriptions;
pub mod version;
mod videos;

use std::sync::Arc;

use application::{
    analytics::AnalyticsSink,
    async_lesson::{
        AiFeedbackGenerator, GetVideoReview, ListCoachVideos, ListMyVideos, ReturnVideoReview,
        ReviewGate, SubmitVideo, VideoCoachDisplayNameLookup, VideoLessonMenuLookup,
        VideoRepository, VideoStorage, VideoStudentDisplayNameLookup,
    },
    catalog::{
        CreateLessonMenu, LessonMenuCoachDisplayNameLookup, LessonMenuRepository,
        ListCoachLessonMenus, ListLessonMenus, ListMyLessonMenus, UpdateLessonMenu,
    },
    clock::Clock,
    domain_event::DomainEventPublisher,
    entitlement::{
        EntitlementRepository, GetEntitlement, ListSubscriptionPlans, StartSubscription,
    },
    identity::{
        AccountRepository, GetAccount, ListStudents, RegisterAccount, StudentContractLookup,
        StudentLastBookingLookup, TokenIssuer, TokenVerifier,
    },
    lesson::{
        GetMyLessonStats, LessonCompletionCoachDisplayNameLookup, LessonCompletionRepository,
        LessonCompletionReviewLookup, ListMyLessonCompletions, MonthlyReviewSummaryLookup,
        RecordLessonCompletion,
    },
    live_lesson::{
        BookingLookup, EndLiveLessonSession, LiveLessonSessionRepository, StartLiveLessonSession,
        VideoCallProvider,
    },
    matching::{
        AddFavoriteCoach, CoachCandidateLookup, CoachReviewSummaryLookup, FavoriteCoachRepository,
        ListCoachesForStudent, ListFavoriteCoaches, PublishedLessonMenuCountLookup,
        RemoveFavoriteCoach, StudentSportLookup, SuspendedCoachLookup,
    },
    messaging::{
        ChatGate, ListMessageThreads, ListMessages, MarkMessageThreadRead, MessageThreadRepository,
        SendMessage,
    },
    notification::{
        AnnouncementRecipientLookup, AnnouncementRepository, GetNotificationSettings,
        ListAnnouncements, ListMyNotifications, MarkMyNotificationsRead, NotificationRepository,
        NotificationSettingsRepository, PushSender, SendAnnouncement, UpdateNotificationSettings,
    },
    profile::{
        CoachAcceptanceLookup, CoachFavoriteLookup, CoachProfileRepository, GetCoachBio,
        GetCoachProfile, ListCoaches, UpdateCoachProfile, UpdateStudentProfile,
    },
    review::{
        CompletedLessonLookup, GetCoachReviewSummary, LessonReviewRepository, ListCoachReviews,
        ListLowRatedReviews, ReplyToLessonReview, ReviewedCoachDisplayNameLookup,
        ReviewedLessonMenuLookup, ReviewerDisplayNameLookup, SubmitLessonReview,
    },
    scheduling::{
        BookingCoachDisplayNameLookup, BookingRepository, CancelBooking, CoachAcceptanceRepository,
        CoachIntroductionLookup, ConfirmBooking, CreateBooking, GetSlotSupply, LessonMenuLookup,
        ListBookings, ListCoachBookings, ListCoachSlots, ListMyBookings, PublishSlot, QuotaGate,
        ResumeCoach, SearchSlots, SlotRepository, StudentDisplayNameLookup, SuspendCoach,
        WithdrawSlot,
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
/// `/bookings`（student-app の予約の作成・確定・キャンセルと、自分の予約の一覧 `GET /bookings/me`）・`POST /subscriptions`（月額契約の開始）・
/// `GET /entitlements/me`（受講権の照会）は生徒だけ。`/live-lesson-sessions`（通話セッションの開始・終了）は
/// 役割で絞らず、予約の参加者（生徒・コーチ）かをユースケースが確かめる（契約に 403 がない）。
/// `POST /videos`（動画の送付）は生徒だけ、`POST /videos/{video_id}/review`（admin-web の添削の返却）はコーチだけ。
/// `GET /videos/{video_id}/review`（添削結果の取得）は役割で絞らず、動画の当事者かをユースケースが確かめる。
/// `POST /lessons/completions`（レッスンの完了記録）も役割で絞らず、通話セッション・動画の当事者かをユースケースが
/// 確かめる（契約に 403 がない）。生徒が受けたレッスンの一覧（`GET /lessons/completions/me`）は [`my_lesson_completions_router`]。`GET /slot-supply`（admin-web のスポーツごとの予約枠の需給）は本部だけ。
/// `/message-threads`（student-app・admin-web の生徒と担当コーチのチャット）は役割で絞らず、スレッドの当事者かを
/// ユースケースが確かめる（生徒がチャットの送り放題を持たない送信だけが 403）。
/// `POST /lesson-reviews`（レッスンの評価）は生徒だけで、評価できるのは自分が生徒の完了したレッスンだけ。
/// `GET /coaches/{coach_id}/review-summary`（admin-web・student-app のコーチの評価の件数と平均）と
/// `GET /coaches/{coach_id}/reviews`（コーチが受けた評価の一覧。評価したレッスンのメニューを添える）は本部・コーチ・生徒で、コーチは自分の分だけ。`GET /coaches`（コーチの一覧）は [`coaches_router`]。
/// `GET /coaches/me/bookings`（admin-web のコーチの自分の予約の一覧）と `GET /coaches/me/slots`（コーチの自分の枠の
/// 一覧）と `GET /coaches/me/videos`（コーチに届いた動画の一覧）はコーチだけ。
/// `GET /me`（admin-web のログイン中の本人のアカウント）は役割で絞らない。`PUT /me/profile`（admin-web の自分の
/// 自己紹介文の変更）はコーチだけ。同じパスの student-app の生徒の分（`GET`・`PATCH /me/profile`）は [`my_profile_router`]。
/// `/notification-settings/me`（student-app の自分の通知の受け取り設定の照会・変更）は生徒だけ。
#[allow(clippy::too_many_arguments)]
pub fn router_with<
    R,
    S,
    J,
    I,
    D,
    B,
    G,
    E,
    L,
    K,
    V,
    W,
    U,
    Y,
    Q,
    F,
    M,
    X,
    H,
    O,
    N,
    Rm,
    Z,
    A,
    P,
    C,
    T,
>(
    register_account: RegisterAccount<R, A, C>,
    get_account: GetAccount<R>,
    get_coach_bio: GetCoachBio<R>,
    update_coach_profile: UpdateCoachProfile<R>,
    publish_slot: PublishSlot<S, J, A, C>,
    search_slots: SearchSlots<S, J, I, C>,
    create_booking: CreateBooking<B, J, A, C>,
    confirm_booking: ConfirmBooking<B, G, A, P, C>,
    cancel_booking: CancelBooking<B, A, P, C>,
    get_slot_supply: GetSlotSupply<B, C>,
    list_my_bookings: ListMyBookings<B, I, G, C>,
    list_coach_bookings: ListCoachBookings<B, D>,
    list_coach_slots: ListCoachSlots<S, J, B>,
    start_subscription: StartSubscription<E, A, P, C>,
    get_entitlement: GetEntitlement<E, C>,
    start_live_lesson_session: StartLiveLessonSession<L, K, V, A, C>,
    end_live_lesson_session: EndLiveLessonSession<L, V, A, C>,
    submit_video: SubmitVideo<W, J, U, Y, A, P, C>,
    get_video_review: GetVideoReview<W>,
    return_video_review: ReturnVideoReview<W, Q, A, P, C>,
    list_coach_videos: ListCoachVideos<W, F>,
    record_lesson_completion: RecordLessonCompletion<L, W, M, A, P, C>,
    list_message_threads: ListMessageThreads<X>,
    list_messages: ListMessages<X>,
    send_message: SendMessage<X, H, A, P, C>,
    mark_message_thread_read: MarkMessageThreadRead<X, C>,
    submit_lesson_review: SubmitLessonReview<O, N, A, C>,
    get_coach_review_summary: GetCoachReviewSummary<N>,
    list_coach_reviews: ListCoachReviews<N, Rm>,
    get_notification_settings: GetNotificationSettings<Z>,
    update_notification_settings: UpdateNotificationSettings<Z>,
    tokens: T,
) -> Router
where
    R: AccountRepository + CoachProfileRepository + 'static,
    S: SlotRepository + 'static,
    J: LessonMenuLookup + VideoLessonMenuLookup + 'static,
    I: CoachIntroductionLookup + 'static,
    D: StudentDisplayNameLookup + 'static,
    B: BookingRepository + 'static,
    G: QuotaGate + 'static,
    E: EntitlementRepository + 'static,
    L: LiveLessonSessionRepository + 'static,
    K: BookingLookup + 'static,
    V: VideoCallProvider + 'static,
    W: VideoRepository + 'static,
    U: VideoStorage + 'static,
    Y: AiFeedbackGenerator + 'static,
    Q: ReviewGate + 'static,
    F: VideoStudentDisplayNameLookup + 'static,
    M: LessonCompletionRepository + 'static,
    X: MessageThreadRepository + 'static,
    H: ChatGate + 'static,
    O: CompletedLessonLookup + 'static,
    N: LessonReviewRepository + 'static,
    Rm: ReviewedLessonMenuLookup + 'static,
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
    let me = Router::new()
        .route("/me", get(me::get_my_account::<R>))
        .route(
            "/me/profile",
            put(me::update_my_coach_profile::<R>).route_layer(require_role(&[Role::Coach])),
        )
        .with_state(Arc::new(me::MeState {
            get_account,
            get_coach_bio,
            update_coach_profile,
        }));
    let slots = Router::new()
        .route(
            "/slots",
            get(slots::search_slots::<S, J, I, A, C>).merge(
                post(slots::publish_slot::<S, J, I, A, C>)
                    .route_layer(require_role(&[Role::Coach])),
            ),
        )
        .with_state(Arc::new(slots::SlotsState {
            publish_slot,
            search_slots,
        }));
    let bookings = Router::new()
        .route(
            "/bookings",
            post(bookings::create_booking::<B, J, G, A, P, C>),
        )
        .route(
            "/bookings/{booking_id}/confirm",
            post(bookings::confirm_booking::<B, J, G, A, P, C>),
        )
        .route(
            "/bookings/{booking_id}/cancel",
            post(bookings::cancel_booking::<B, J, G, A, P, C>),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(bookings::BookingsState {
            create_booking,
            confirm_booking,
            cancel_booking,
        }));
    let my_bookings = Router::new()
        .route(
            "/bookings/me",
            get(my_bookings::list_my_bookings::<B, I, G, C>),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(my_bookings::MyBookingsState { list_my_bookings }));
    let coach_bookings = Router::new()
        .route(
            "/coaches/me/bookings",
            get(coach_bookings::list_my_coach_bookings::<B, D>),
        )
        .route_layer(require_role(&[Role::Coach]))
        .with_state(Arc::new(coach_bookings::CoachBookingsState {
            list_coach_bookings,
        }));
    let coach_slots = Router::new()
        .route(
            "/coaches/me/slots",
            get(coach_slots::list_my_coach_slots::<S, J, B>),
        )
        .route_layer(require_role(&[Role::Coach]))
        .with_state(Arc::new(coach_slots::CoachSlotsState { list_coach_slots }));
    let coach_videos = Router::new()
        .route(
            "/coaches/me/videos",
            get(coach_videos::list_my_coach_videos::<W, F>),
        )
        .route_layer(require_role(&[Role::Coach]))
        .with_state(Arc::new(coach_videos::CoachVideosState {
            list_coach_videos,
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
            post(videos::submit_video::<W, J, U, Y, Q, A, P, C>)
                .route_layer(require_role(&[Role::Student])),
        )
        .route(
            "/videos/{video_id}/review",
            get(videos::get_video_review::<W, J, U, Y, Q, A, P, C>).merge(
                post(videos::return_video_review::<W, J, U, Y, Q, A, P, C>)
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
    let reviews =
        Router::new()
            .route(
                "/lesson-reviews",
                post(reviews::submit_lesson_review::<O, N, Rm, A, C>)
                    .route_layer(require_role(&[Role::Student])),
            )
            .route(
                "/coaches/{coach_id}/review-summary",
                get(reviews::get_coach_review_summary::<O, N, Rm, A, C>).route_layer(require_role(
                    &[Role::Headquarters, Role::Coach, Role::Student],
                )),
            )
            .route(
                "/coaches/{coach_id}/reviews",
                get(reviews::list_coach_reviews::<O, N, Rm, A, C>).route_layer(require_role(&[
                    Role::Headquarters,
                    Role::Coach,
                    Role::Student,
                ])),
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
        .merge(me)
        .merge(slots)
        .merge(bookings)
        .merge(my_bookings)
        .merge(coach_bookings)
        .merge(coach_slots)
        .merge(coach_videos)
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

/// コーチの一覧（`GET /coaches`）のルーター。[`router_with`] と同じくトークンの検証の層を重ねるので、組み立て用の
/// クレートは [`router_with`] のルーターに `merge` する（同じ検証器を渡す）。
///
/// admin-web の `listCoaches`（本部）と student-app の `listCoachesForStudent`（生徒）が同じパスなので、1 つの
/// ハンドラーがトークンの役割で返す形を決める。コーチは 403。
pub fn coaches_router<R, A, S, C, V, M, F, U, T>(
    list_coaches: ListCoaches<R, A>,
    list_coaches_for_student: ListCoachesForStudent<S, C, V, M, F, U>,
    tokens: T,
) -> Router
where
    R: AccountRepository + 'static,
    A: CoachAcceptanceLookup + 'static,
    S: StudentSportLookup + 'static,
    C: CoachCandidateLookup + 'static,
    V: CoachReviewSummaryLookup + 'static,
    M: PublishedLessonMenuCountLookup + 'static,
    F: FavoriteCoachRepository + 'static,
    U: SuspendedCoachLookup + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/coaches",
            get(coaches::list_coaches::<R, A, S, C, V, M, F, U>),
        )
        .route_layer(require_role(&[Role::Headquarters, Role::Student]))
        .with_state(Arc::new(coaches::CoachesState {
            list_coaches,
            list_coaches_for_student,
        }))
        .layer(auth::TokenVerification::layer(tokens))
}

/// catalog（コーチのレッスンメニュー）のルーター。[`router_with`] と同じくトークンの検証の層を重ねるので、組み立て用の
/// クレートは [`router_with`] のルーターに `merge` する（同じ検証器を渡す）。
///
/// `/coaches/me/lesson-menus`（admin-web の自分のメニューの一覧・作成）と `PUT /coaches/me/lesson-menus/{lesson_menu_id}`
/// （変更・非公開）はコーチだけで、扱うのはトークンのコーチ自身のメニューだけ。
/// `GET /coaches/{coach_id}/lesson-menus`（student-app のコーチの公開中のメニューの一覧）は生徒だけ。
/// `GET /lesson-menus`（admin-web のすべてのコーチのメニューの横断の一覧）は本部だけ。
pub fn catalog_router<M, R, C, D, T>(
    create_lesson_menu: CreateLessonMenu<M, R, C>,
    update_lesson_menu: UpdateLessonMenu<M, R>,
    list_my_lesson_menus: ListMyLessonMenus<M>,
    list_coach_lesson_menus: ListCoachLessonMenus<M>,
    list_lesson_menus: ListLessonMenus<M, D>,
    tokens: T,
) -> Router
where
    M: LessonMenuRepository + 'static,
    R: AccountRepository + 'static,
    C: Clock + 'static,
    D: LessonMenuCoachDisplayNameLookup + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    let coach_routes = Router::new()
        .route(
            "/coaches/me/lesson-menus",
            get(lesson_menus::list_my_lesson_menus::<M, R, C, D>)
                .post(lesson_menus::create_my_lesson_menu::<M, R, C, D>),
        )
        .route(
            "/coaches/me/lesson-menus/{lesson_menu_id}",
            put(lesson_menus::update_my_lesson_menu::<M, R, C, D>),
        )
        .route_layer(require_role(&[Role::Coach]));
    let student_routes = Router::new()
        .route(
            "/coaches/{coach_id}/lesson-menus",
            get(lesson_menus::list_coach_lesson_menus::<M, R, C, D>),
        )
        .route_layer(require_role(&[Role::Student]));
    let headquarters_routes = Router::new()
        .route(
            "/lesson-menus",
            get(lesson_menus::list_lesson_menus::<M, R, C, D>),
        )
        .route_layer(require_role(&[Role::Headquarters]));
    coach_routes
        .merge(student_routes)
        .merge(headquarters_routes)
        .with_state(Arc::new(lesson_menus::LessonMenusState {
            create_lesson_menu,
            update_lesson_menu,
            list_my_lesson_menus,
            list_coach_lesson_menus,
            list_lesson_menus,
        }))
        .layer(auth::TokenVerification::layer(tokens))
}

/// コーチの紹介（student-app の `GET /coaches/{coach_id}`）のルーター。[`router_with`] と同じくトークンの検証の層を
/// 重ねるので、組み立て用のクレートは [`router_with`] のルーターに `merge` する（同じ検証器を渡す）。
///
/// `GET /coaches/{coach_id}` は生徒だけ。
pub fn coach_profile_router<R, F, T>(get_coach_profile: GetCoachProfile<R, F>, tokens: T) -> Router
where
    R: AccountRepository + CoachProfileRepository + 'static,
    F: CoachFavoriteLookup + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/coaches/{coach_id}",
            get(coach_profiles::get_coach_profile::<R, F>),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(coach_profiles::CoachProfilesState {
            get_coach_profile,
        }))
        .layer(auth::TokenVerification::layer(tokens))
}

/// 生徒のお気に入りのコーチ（student-app の `GET /favorite-coaches`・`PUT`・`DELETE /favorite-coaches/{coach_id}`）の
/// ルーター。[`router_with`] と同じくトークンの検証の層を重ねるので、組み立て用のクレートは [`router_with`] のルーターに
/// `merge` する（同じ検証器を渡す）。
///
/// どれも生徒だけ（コーチ・本部は 403）で、扱うのはトークンの生徒自身のお気に入りだけ。
pub fn favorite_coaches_router<F, R, C, V, T>(
    add_favorite_coach: AddFavoriteCoach<F, R, C>,
    remove_favorite_coach: RemoveFavoriteCoach<F>,
    list_favorite_coaches: ListFavoriteCoaches<F, R, V>,
    tokens: T,
) -> Router
where
    F: FavoriteCoachRepository + 'static,
    R: AccountRepository + 'static,
    C: Clock + 'static,
    V: CoachReviewSummaryLookup + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/favorite-coaches",
            get(favorite_coaches::list_favorite_coaches::<F, R, C, V>),
        )
        .route(
            "/favorite-coaches/{coach_id}",
            put(favorite_coaches::add_favorite_coach::<F, R, C, V>)
                .delete(favorite_coaches::remove_favorite_coach::<F, R, C, V>),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(favorite_coaches::FavoriteCoachesState {
            add_favorite_coach,
            remove_favorite_coach,
            list_favorite_coaches,
        }))
        .layer(auth::TokenVerification::layer(tokens))
}

/// 生徒の自分のプロフィール（student-app の `GET /me/profile`・`PATCH /me/profile`）のルーター。[`router_with`] と同じく
/// トークンの検証の層を重ねるので、組み立て用のクレートは [`router_with`] のルーターに `merge` する（同じ検証器を渡す）。
/// 同じパスの `PUT /me/profile`（admin-web のコーチの自己紹介文）は [`router_with`] にあり、`merge` で 1 つのパスにまとまる。
///
/// `GET`・`PATCH /me/profile` は生徒だけで、扱うのはトークンの生徒自身のアカウントだけ。
pub fn my_profile_router<R, T>(
    get_account: GetAccount<R>,
    update_student_profile: UpdateStudentProfile<R>,
    tokens: T,
) -> Router
where
    R: AccountRepository + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/me/profile",
            get(my_profile::get_my_student_profile::<R>)
                .patch(my_profile::update_my_student_profile::<R>),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(my_profile::MyProfileState {
            get_account,
            update_student_profile,
        }))
        .layer(auth::TokenVerification::layer(tokens))
}

/// 生徒が送った動画の一覧（student-app の `GET /videos/me`）のルーター。[`router_with`] と同じくトークンの検証の層を
/// 重ねるので、組み立て用のクレートは [`router_with`] のルーターに `merge` する（同じ検証器を渡す）。
///
/// `GET /videos/me` は生徒だけで、照会するのはトークンの生徒自身が送った動画だけ。
pub fn my_videos_router<V, D, T>(list_my_videos: ListMyVideos<V, D>, tokens: T) -> Router
where
    V: VideoRepository + 'static,
    D: VideoCoachDisplayNameLookup + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route("/videos/me", get(my_videos::list_my_videos::<V, D>))
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(my_videos::MyVideosState { list_my_videos }))
        .layer(auth::TokenVerification::layer(tokens))
}

/// 生徒が受けたレッスンの一覧（student-app の `GET /lessons/completions/me`）のルーター。[`router_with`] と同じくトークンの
/// 検証の層を重ねるので、組み立て用のクレートは [`router_with`] のルーターに `merge` する（同じ検証器を渡す）。
///
/// `GET /lessons/completions/me` は生徒だけ（コーチ・本部は 403）。同じ接頭辞の `POST /lessons/completions`（完了記録）は
/// [`router_with`] で、役割で絞らない。
pub fn my_lesson_completions_router<R, L, B, W, D, V, T>(
    list_my_lesson_completions: ListMyLessonCompletions<R, L, B, W, D, V>,
    tokens: T,
) -> Router
where
    R: LessonCompletionRepository + 'static,
    L: LiveLessonSessionRepository + 'static,
    B: BookingLookup + 'static,
    W: VideoRepository + 'static,
    D: LessonCompletionCoachDisplayNameLookup + 'static,
    V: LessonCompletionReviewLookup + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/lessons/completions/me",
            get(my_lesson_completions::list_my_lesson_completions::<R, L, B, W, D, V>),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(my_lesson_completions::MyLessonCompletionsState {
            list_my_lesson_completions,
        }))
        .layer(auth::TokenVerification::layer(tokens))
}

/// 生徒とコーチのお知らせ（student-app・admin-web の `GET /notifications/me`・`POST /notifications/me/read`）のルーター。
/// [`router_with`] と同じくトークンの検証の層を重ねるので、組み立て用のクレートは [`router_with`] のルーターに `merge` する
/// （同じ検証器を渡す）。
///
/// どちらも生徒とコーチだけ（本部は 403）で、扱うのはトークンのアカウント自身が受け取るお知らせだけ。
pub fn notifications_router<N, A, C, T>(
    list_my_notifications: ListMyNotifications<N, A>,
    mark_my_notifications_read: MarkMyNotificationsRead<N, A, C>,
    tokens: T,
) -> Router
where
    N: NotificationRepository + 'static,
    A: AnnouncementRepository + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/notifications/me",
            get(notifications::list_my_notifications::<N, A, C>),
        )
        .route(
            "/notifications/me/read",
            post(notifications::mark_my_notifications_read::<N, A, C>),
        )
        .route_layer(require_role(&[Role::Student, Role::Coach]))
        .with_state(Arc::new(notifications::NotificationsState {
            list_my_notifications,
            mark_my_notifications_read,
        }))
        .layer(auth::TokenVerification::layer(tokens))
}

/// 契約プランの一覧（student-app の `GET /subscription-plans`）のルーター。[`router_with`] と同じくトークンの検証の層を
/// 重ねるので、組み立て用のクレートは [`router_with`] のルーターに `merge` する（同じ検証器を渡す）。
///
/// `GET /subscription-plans` は生徒だけ。
pub fn subscription_plans_router<T>(
    list_subscription_plans: ListSubscriptionPlans,
    tokens: T,
) -> Router
where
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/subscription-plans",
            get(subscription_plans::list_subscription_plans),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(subscription_plans::SubscriptionPlansState {
            list_subscription_plans,
        }))
        .layer(auth::TokenVerification::layer(tokens))
}

/// コーチが公開した枠の取り下げ（admin-web の `POST /coaches/me/slots/{slot_id}/withdraw`）のルーター。
/// [`router_with`] と同じくトークンの検証の層を重ねるので、組み立て用のクレートは [`router_with`] のルーターに `merge`
/// する（同じ検証器を渡す）。
///
/// `POST /coaches/me/slots/{slot_id}/withdraw` はコーチだけで、取り下げられるのはトークンのコーチ自身の枠だけ。
pub fn slot_withdrawal_router<S, A, C, T>(withdraw_slot: WithdrawSlot<S, A, C>, tokens: T) -> Router
where
    S: SlotRepository + 'static,
    A: AnalyticsSink + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/coaches/me/slots/{slot_id}/withdraw",
            post(slot_withdrawals::withdraw_my_slot::<S, A, C>),
        )
        .route_layer(require_role(&[Role::Coach]))
        .with_state(Arc::new(slot_withdrawals::SlotWithdrawalsState {
            withdraw_slot,
        }))
        .layer(auth::TokenVerification::layer(tokens))
}

/// 本部によるコーチの受付の停止・再開（admin-web の `POST /coaches/{coach_id}/suspend`・`POST /coaches/{coach_id}/resume`）
/// のルーター。[`router_with`] と同じくトークンの検証の層を重ねるので、組み立て用のクレートは [`router_with`] のルーターに
/// `merge` する（同じ検証器を渡す）。
///
/// どちらも本部だけ（コーチ・生徒は 403）。
pub fn coach_acceptance_router<R, I, T>(
    suspend_coach: SuspendCoach<R, I>,
    resume_coach: ResumeCoach<R, I>,
    tokens: T,
) -> Router
where
    R: CoachAcceptanceRepository + 'static,
    I: CoachIntroductionLookup + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/coaches/{coach_id}/suspend",
            post(coach_acceptances::suspend_coach::<R, I>),
        )
        .route(
            "/coaches/{coach_id}/resume",
            post(coach_acceptances::resume_coach::<R, I>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(coach_acceptances::CoachAcceptancesState {
            suspend_coach,
            resume_coach,
        }))
        .layer(auth::TokenVerification::layer(tokens))
}

/// コーチの今月と先月のレッスンの実績（admin-web の `GET /coaches/me/lesson-stats`）のルーター。[`router_with`] と
/// 同じくトークンの検証の層を重ねるので、組み立て用のクレートは [`router_with`] のルーターに `merge` する（同じ検証器を渡す）。
///
/// `GET /coaches/me/lesson-stats` はコーチだけで（本部・生徒は 403）、照会するのはトークンのコーチ自身の実績だけ。
pub fn lesson_stats_router<M, R, C, T>(
    get_my_lesson_stats: GetMyLessonStats<M, R, C>,
    tokens: T,
) -> Router
where
    M: LessonCompletionRepository + 'static,
    R: MonthlyReviewSummaryLookup + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/coaches/me/lesson-stats",
            get(lesson_stats::get_my_lesson_stats::<M, R, C>),
        )
        .route_layer(require_role(&[Role::Coach]))
        .with_state(Arc::new(lesson_stats::LessonStatsState {
            get_my_lesson_stats,
        }))
        .layer(auth::TokenVerification::layer(tokens))
}

/// 本部が見る予約の一覧（admin-web の `GET /bookings`）のルーター。[`router_with`] と同じくトークンの検証の層を重ねるので、
/// 組み立て用のクレートは [`router_with`] のルーターに `merge` する（同じ検証器を渡す）。同じパスの `POST /bookings`
/// （student-app の予約の作成。生徒だけ）は [`router_with`] にあり、`merge` で 1 つのパスにまとまる。
///
/// `GET /bookings` は本部だけ（コーチ・生徒は 403）。
pub fn headquarters_bookings_router<B, K, D, C, T>(
    list_bookings: ListBookings<B, K, D, C>,
    tokens: T,
) -> Router
where
    B: BookingRepository + 'static,
    K: BookingCoachDisplayNameLookup + 'static,
    D: StudentDisplayNameLookup + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/bookings",
            get(headquarters_bookings::list_bookings::<B, K, D, C>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(headquarters_bookings::HeadquartersBookingsState {
            list_bookings,
        }))
        .layer(auth::TokenVerification::layer(tokens))
}

/// 本部が見る生徒の一覧（admin-web の `GET /students`）のルーター。[`router_with`] と同じくトークンの検証の層を
/// 重ねるので、組み立て用のクレートは [`router_with`] のルーターに `merge` する（同じ検証器を渡す）。
///
/// `GET /students` は本部だけ（コーチ・生徒は 403）。
pub fn students_router<R, C, B, T>(list_students: ListStudents<R, C, B>, tokens: T) -> Router
where
    R: AccountRepository + 'static,
    C: StudentContractLookup + 'static,
    B: StudentLastBookingLookup + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route("/students", get(students::list_students::<R, C, B>))
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(students::StudentsState { list_students }))
        .layer(auth::TokenVerification::layer(tokens))
}

/// 本部からのお知らせ（admin-web の `POST /announcements`・`GET /announcements`）のルーター。[`router_with`] と同じく
/// トークンの検証の層を重ねるので、組み立て用のクレートは [`router_with`] のルーターに `merge` する（同じ検証器を渡す）。
///
/// どちらも本部だけ（コーチ・生徒は 403）。受け取った生徒は自分のお知らせ（[`notifications_router`]）で読む。
pub fn announcements_router<A, L, N, S, P, C, T>(
    send_announcement: SendAnnouncement<A, L, N, S, P, C>,
    list_announcements: ListAnnouncements<A>,
    tokens: T,
) -> Router
where
    A: AnnouncementRepository + 'static,
    L: AnnouncementRecipientLookup + 'static,
    N: NotificationRepository + 'static,
    S: NotificationSettingsRepository + 'static,
    P: PushSender + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/announcements",
            post(announcements::send_announcement::<A, L, N, S, P, C>)
                .get(announcements::list_announcements::<A, L, N, S, P, C>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(announcements::AnnouncementsState {
            send_announcement,
            list_announcements,
        }))
        .layer(auth::TokenVerification::layer(tokens))
}

/// 本部が見る低い評価の一覧（admin-web の `GET /lesson-reviews/low`）のルーター。[`router_with`] と同じくトークンの
/// 検証の層を重ねるので、組み立て用のクレートは [`router_with`] のルーターに `merge` する（同じ検証器を渡す）。
///
/// `GET /lesson-reviews/low` は本部だけ（コーチ・生徒は 403）。同じ接頭辞の `POST /lesson-reviews`（評価）は
/// [`router_with`] で、生徒だけ。
pub fn low_rated_reviews_router<R, M, D, S, C, T>(
    list_low_rated_reviews: ListLowRatedReviews<R, M, D, S, C>,
    tokens: T,
) -> Router
where
    R: LessonReviewRepository + 'static,
    M: ReviewedLessonMenuLookup + 'static,
    D: ReviewedCoachDisplayNameLookup + 'static,
    S: ReviewerDisplayNameLookup + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/lesson-reviews/low",
            get(low_rated_reviews::list_low_rated_reviews::<R, M, D, S, C>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(low_rated_reviews::LowRatedReviewsState {
            list_low_rated_reviews,
        }))
        .layer(auth::TokenVerification::layer(tokens))
}

/// 評価への返信（admin-web の `PUT /lesson-reviews/{review_id}/reply`）のルーター。[`router_with`] と同じくトークンの
/// 検証の層を重ねるので、組み立て用のクレートは [`router_with`] のルーターに `merge` する（同じ検証器を渡す）。
///
/// コーチだけ（本部・生徒は 403）で、返信できるのは評価されたコーチだけ（他のコーチも 403。ユースケースが確かめる）。
pub fn review_replies_router<R, C, T>(
    reply_to_lesson_review: ReplyToLessonReview<R, C>,
    tokens: T,
) -> Router
where
    R: LessonReviewRepository + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/lesson-reviews/{review_id}/reply",
            put(review_replies::reply_to_lesson_review::<R, C>),
        )
        .route_layer(require_role(&[Role::Coach]))
        .with_state(Arc::new(review_replies::ReviewRepliesState {
            reply_to_lesson_review,
        }))
        .layer(auth::TokenVerification::layer(tokens))
}

/// 死活監視。プロセスが要求を受け付けられれば 200 を返す。
async fn healthz() -> (StatusCode, &'static str) {
    (StatusCode::OK, "ok")
}
