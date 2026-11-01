//! 認証・予約・受講権・通話・動画・チャット・評価などの機能のルーターを 1 つにまとめる [`router_with`]。
//!
//! 機能ごとのルーターに分ける前の入口で、引数の並びを変えずに残している（interface の統合テストが使う）。組み立て用の
//! クレート（`server`）は機能ごとのルーターを直接 `merge` する。新しい機能はここに足さず、機能ごとのルーターにする。

use application::{
    analytics::AnalyticsSink,
    async_lesson::{
        AiFeedbackGenerator, GetVideoReview, ListCoachVideos, ReturnVideoReview, ReviewGate,
        SubmitVideo, VideoLessonMenuLookup, VideoRepository, VideoStorage,
        VideoStudentDisplayNameLookup,
    },
    clock::Clock,
    domain_event::DomainEventPublisher,
    entitlement::{EntitlementRepository, GetEntitlement, StartSubscription},
    identity::{AccountRepository, GetAccount, RegisterAccount, TokenIssuer, TokenVerifier},
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
    profile::{CoachProfileRepository, GetCoachBio, UpdateCoachProfile},
    review::{
        CompletedLessonLookup, GetCoachReviewSummary, LessonReviewRepository, ListCoachReviews,
        ReviewedLessonMenuLookup, SubmitLessonReview,
    },
    scheduling::{
        BookingRepository, CancelBooking, CoachIntroductionLookup, ConfirmBooking, CreateBooking,
        GetSlotSupply, LessonMenuLookup, ListCoachBookings, ListCoachSlots, ListMyBookings,
        PublishSlot, QuotaGate, SearchSlots, SlotRepository, StudentDisplayNameLookup,
    },
};
use axum::Router;

use crate::shared_tokens::SharedTokens;

/// ユースケースとアクセストークンのポート（発行・検証）を外から渡して、次の機能のルーターを `merge` したルーター。
/// どのパスをどの役割が呼べるかは、それぞれの機能のルーターに書く。
///
/// [`crate::healthz_router`]・[`crate::accounts_router`]・[`crate::me_router`]・[`crate::slots_router`]・
/// [`crate::bookings_router`]・[`crate::my_bookings_router`]・[`crate::coach_bookings_router`]・
/// [`crate::coach_slots_router`]・[`crate::coach_videos_router`]・[`crate::slot_supply_router`]・
/// [`crate::subscriptions_router`]・[`crate::entitlements_router`]・[`crate::live_lesson_sessions_router`]・
/// [`crate::videos_router`]・[`crate::lessons_router`]・[`crate::messaging_router`]・[`crate::reviews_router`]・
/// [`crate::notification_settings_router`]
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
    let tokens = SharedTokens::new(tokens);
    Router::new()
        .merge(crate::accounts_router(register_account, tokens.clone()))
        .merge(crate::bookings_router(
            create_booking,
            confirm_booking,
            cancel_booking,
            tokens.clone(),
        ))
        .merge(crate::coach_bookings_router(
            list_coach_bookings,
            tokens.clone(),
        ))
        .merge(crate::coach_slots_router(list_coach_slots, tokens.clone()))
        .merge(crate::coach_videos_router(
            list_coach_videos,
            tokens.clone(),
        ))
        .merge(crate::entitlements_router(get_entitlement, tokens.clone()))
        .merge(crate::healthz_router())
        .merge(crate::lessons_router(
            record_lesson_completion,
            tokens.clone(),
        ))
        .merge(crate::live_lesson_sessions_router(
            start_live_lesson_session,
            end_live_lesson_session,
            tokens.clone(),
        ))
        .merge(crate::me_router(
            get_account,
            get_coach_bio,
            update_coach_profile,
            tokens.clone(),
        ))
        .merge(crate::messaging_router(
            list_message_threads,
            list_messages,
            send_message,
            mark_message_thread_read,
            tokens.clone(),
        ))
        .merge(crate::my_bookings_router(list_my_bookings, tokens.clone()))
        .merge(crate::notification_settings_router(
            get_notification_settings,
            update_notification_settings,
            tokens.clone(),
        ))
        .merge(crate::reviews_router(
            submit_lesson_review,
            get_coach_review_summary,
            list_coach_reviews,
            tokens.clone(),
        ))
        .merge(crate::slot_supply_router(get_slot_supply, tokens.clone()))
        .merge(crate::slots_router(
            publish_slot,
            search_slots,
            tokens.clone(),
        ))
        .merge(crate::subscriptions_router(
            start_subscription,
            tokens.clone(),
        ))
        .merge(crate::videos_router(
            submit_video,
            get_video_review,
            return_video_review,
            tokens,
        ))
}
