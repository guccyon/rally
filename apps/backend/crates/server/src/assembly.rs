//! [`Ports`] の実装。集約ごとの保存先と、時計・行動トラッキング・通話基盤・境界間イベントの発行先をまとめて持つ。

use application::{
    analytics::AnalyticsSink,
    async_lesson::VideoRepository,
    catalog::LessonMenuRepository,
    clock::Clock,
    domain_event::DomainEventPublisher,
    entitlement::EntitlementRepository,
    identity::AccountRepository,
    lesson::LessonCompletionRepository,
    live_lesson::{LiveLessonSessionRepository, VideoCallProvider},
    matching::FavoriteCoachRepository,
    messaging::MessageThreadRepository,
    notification::{
        AnnouncementDraftRepository, AnnouncementRepository, NotificationRepository,
        NotificationSettingsRepository, PushSender,
    },
    profile::CoachProfileRepository,
    review::LessonReviewRepository,
    scheduling::{BookingRepository, CoachAcceptanceRepository, SlotRepository},
};

use crate::{Repositories, ports::Ports};

/// 保存先（[`Repositories`]）と、保存先のほかのポートの実装。
#[allow(clippy::type_complexity)]
pub(crate) struct Assembly<R, S, B, E, L, W, M, X, N, Z, K, J, Q, G, H, F, C, A, V, P, D> {
    pub(crate) repositories: Repositories<R, S, B, E, L, W, M, X, N, Z, K, J, Q, G, H, F>,
    pub(crate) clock: C,
    pub(crate) analytics: A,
    pub(crate) video_call: V,
    pub(crate) push: P,
    pub(crate) events: D,
}

impl<R, S, B, E, L, W, M, X, N, Z, K, J, Q, G, H, F, C, A, V, P, D> Ports
    for Assembly<R, S, B, E, L, W, M, X, N, Z, K, J, Q, G, H, F, C, A, V, P, D>
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
    V: VideoCallProvider + Clone + 'static,
    P: PushSender + Clone + 'static,
    D: DomainEventPublisher + Clone + 'static,
{
    type Accounts = R;
    type Slots = S;
    type Bookings = B;
    type Entitlements = E;
    type Sessions = L;
    type Videos = W;
    type Completions = M;
    type Threads = X;
    type Reviews = N;
    type NotificationSettings = Z;
    type Notifications = K;
    type LessonMenus = J;
    type CoachAcceptances = Q;
    type FavoriteCoaches = G;
    type Announcements = H;
    type AnnouncementDrafts = F;
    type Clock = C;
    type Analytics = A;
    type VideoCall = V;
    type Push = P;
    type Events = D;

    fn accounts(&self) -> R {
        self.repositories.accounts.clone()
    }

    fn slots(&self) -> S {
        self.repositories.slots.clone()
    }

    fn bookings(&self) -> B {
        self.repositories.bookings.clone()
    }

    fn entitlements(&self) -> E {
        self.repositories.entitlements.clone()
    }

    fn sessions(&self) -> L {
        self.repositories.sessions.clone()
    }

    fn videos(&self) -> W {
        self.repositories.videos.clone()
    }

    fn completions(&self) -> M {
        self.repositories.completions.clone()
    }

    fn threads(&self) -> X {
        self.repositories.threads.clone()
    }

    fn reviews(&self) -> N {
        self.repositories.reviews.clone()
    }

    fn notification_settings(&self) -> Z {
        self.repositories.notification_settings.clone()
    }

    fn notifications(&self) -> K {
        self.repositories.notifications.clone()
    }

    fn lesson_menus(&self) -> J {
        self.repositories.lesson_menus.clone()
    }

    fn coach_acceptances(&self) -> Q {
        self.repositories.coach_acceptances.clone()
    }

    fn favorite_coaches(&self) -> G {
        self.repositories.favorite_coaches.clone()
    }

    fn announcements(&self) -> H {
        self.repositories.announcements.clone()
    }

    fn announcement_drafts(&self) -> F {
        self.repositories.announcement_drafts.clone()
    }

    fn clock(&self) -> C {
        self.clock.clone()
    }

    fn analytics(&self) -> A {
        self.analytics.clone()
    }

    fn video_call(&self) -> V {
        self.video_call.clone()
    }

    fn push(&self) -> P {
        self.push.clone()
    }

    fn events(&self) -> D {
        self.events.clone()
    }
}
