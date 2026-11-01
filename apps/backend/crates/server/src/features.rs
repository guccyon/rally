//! 機能ごとの組み立て。1 つの機能が 1 つのファイル（`features/<機能>.rs`）で、interface の機能のルーターにポートの実装を
//! 渡す `router` を持つ。
//!
//! 新しい機能はファイルを足し、`mod` の並びと [`router`] の `merge` の並びに 1 行ずつ足す（どちらも名前の順）。

mod accounts;
mod announcements;
mod bookings;
mod coach_acceptances;
mod coach_bookings;
mod coach_overviews;
mod coach_profiles;
mod coach_slots;
mod coach_videos;
mod coaches;
mod entitlements;
mod favorite_coaches;
mod headquarters_bookings;
mod healthz;
mod lesson_menu_publications;
mod lesson_menus;
mod lesson_stats;
mod lessons;
mod live_lesson_sessions;
mod low_rated_review_handlings;
mod low_rated_reviews;
mod me;
mod messaging;
mod my_bookings;
mod my_lesson_completions;
mod my_profile;
mod my_videos;
mod notification_settings;
mod notifications;
mod overdue_live_lesson_sessions;
mod pending_video_reviews;
mod review_replies;
mod reviews;
mod slot_supply;
mod slot_withdrawals;
mod slots;
mod student_activity;
mod students;
mod subscription_plans;
mod subscriptions;
mod unanswered_message_threads;
mod videos;

use axum::Router;

use crate::ports::Ports;

/// すべての機能のルーターを `merge` したルーター。
pub(crate) fn router<P: Ports>(ports: &P) -> Router {
    Router::new()
        .merge(accounts::router(ports))
        .merge(announcements::router(ports))
        .merge(bookings::router(ports))
        .merge(coach_acceptances::router(ports))
        .merge(coach_bookings::router(ports))
        .merge(coach_overviews::router(ports))
        .merge(coach_profiles::router(ports))
        .merge(coach_slots::router(ports))
        .merge(coach_videos::router(ports))
        .merge(coaches::router(ports))
        .merge(entitlements::router(ports))
        .merge(favorite_coaches::router(ports))
        .merge(headquarters_bookings::router(ports))
        .merge(healthz::router(ports))
        .merge(lesson_menu_publications::router(ports))
        .merge(lesson_menus::router(ports))
        .merge(lesson_stats::router(ports))
        .merge(lessons::router(ports))
        .merge(live_lesson_sessions::router(ports))
        .merge(low_rated_review_handlings::router(ports))
        .merge(low_rated_reviews::router(ports))
        .merge(me::router(ports))
        .merge(messaging::router(ports))
        .merge(my_bookings::router(ports))
        .merge(my_lesson_completions::router(ports))
        .merge(my_profile::router(ports))
        .merge(my_videos::router(ports))
        .merge(notification_settings::router(ports))
        .merge(notifications::router(ports))
        .merge(overdue_live_lesson_sessions::router(ports))
        .merge(pending_video_reviews::router(ports))
        .merge(review_replies::router(ports))
        .merge(reviews::router(ports))
        .merge(slot_supply::router(ports))
        .merge(slot_withdrawals::router(ports))
        .merge(slots::router(ports))
        .merge(student_activity::router(ports))
        .merge(students::router(ports))
        .merge(subscription_plans::router(ports))
        .merge(subscriptions::router(ports))
        .merge(unanswered_message_threads::router(ports))
        .merge(videos::router(ports))
}
