//! 機能ごとの組み立て（`features`）が受け取る、ポートの実装の一覧。

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
        AnnouncementRepository, NotificationRepository, NotificationSettingsRepository, PushSender,
    },
    profile::CoachProfileRepository,
    review::LessonReviewRepository,
    scheduling::{BookingRepository, CoachAcceptanceRepository, SlotRepository},
};

/// 機能ごとの組み立てに渡すポートの実装。どれも複製を返し、複製は同じ中身を読み書きする。
///
/// 型を関連型にまとめて、機能ごとの組み立ての関数が型引数を 1 つ（`P: Ports`）だけ取ればよいようにする。
pub(crate) trait Ports {
    type Accounts: AccountRepository + CoachProfileRepository + Clone + 'static;
    type Slots: SlotRepository + Clone + 'static;
    type Bookings: BookingRepository + Clone + 'static;
    type Entitlements: EntitlementRepository + Clone + 'static;
    type Sessions: LiveLessonSessionRepository + Clone + 'static;
    type Videos: VideoRepository + Clone + 'static;
    type Completions: LessonCompletionRepository + Clone + 'static;
    type Threads: MessageThreadRepository + Clone + 'static;
    type Reviews: LessonReviewRepository + Clone + 'static;
    type NotificationSettings: NotificationSettingsRepository + Clone + 'static;
    type Notifications: NotificationRepository + Clone + 'static;
    type LessonMenus: LessonMenuRepository + Clone + 'static;
    type CoachAcceptances: CoachAcceptanceRepository + Clone + 'static;
    type FavoriteCoaches: FavoriteCoachRepository + Clone + 'static;
    type Announcements: AnnouncementRepository + Clone + 'static;
    type Clock: Clock + Clone + 'static;
    type Analytics: AnalyticsSink + Clone + 'static;
    type VideoCall: VideoCallProvider + Clone + 'static;
    type Push: PushSender + Clone + 'static;
    type Events: DomainEventPublisher + Clone + 'static;

    fn accounts(&self) -> Self::Accounts;
    fn slots(&self) -> Self::Slots;
    fn bookings(&self) -> Self::Bookings;
    fn entitlements(&self) -> Self::Entitlements;
    fn sessions(&self) -> Self::Sessions;
    fn videos(&self) -> Self::Videos;
    fn completions(&self) -> Self::Completions;
    fn threads(&self) -> Self::Threads;
    fn reviews(&self) -> Self::Reviews;
    fn notification_settings(&self) -> Self::NotificationSettings;
    fn notifications(&self) -> Self::Notifications;
    fn lesson_menus(&self) -> Self::LessonMenus;
    fn coach_acceptances(&self) -> Self::CoachAcceptances;
    fn favorite_coaches(&self) -> Self::FavoriteCoaches;
    /// 本部が送ったお知らせの記録。
    fn announcements(&self) -> Self::Announcements;
    fn clock(&self) -> Self::Clock;
    fn analytics(&self) -> Self::Analytics;
    /// 通話基盤。
    fn video_call(&self) -> Self::VideoCall;
    /// プッシュ通知の送り先（境界間イベントの購読と、本部からのお知らせが同じ送り先を使う）。
    fn push(&self) -> Self::Push;
    /// 境界間イベントの発行先（インメモリのイベントバス）。
    fn events(&self) -> Self::Events;
}
