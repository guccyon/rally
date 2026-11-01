//! インターフェース層。axum の HTTP ルーティングを置く。
//!
//! 依存してよいのは application と domain。infra には直接依存しない
//! （docs/adr/0003-backend-layers.md）。ポートの実装は組み立て用のクレート（`server`）が渡す。
//!
//! 機能ごとのルーターの関数（`<機能>_router`）は、その機能のハンドラーと同じモジュールに置き、ここでは再公開だけする。
//! 新しい機能はモジュールのファイルを足し、`mod` の並びと `pub use` の並びに 1 行ずつ足す（どちらも名前の順）。
//! どのパスをどの役割が呼べるかは、それぞれのルーターの関数に書く。API の一覧は契約
//! （`specs/contracts/api/<app>/openapi.yaml`）が正。

mod accounts;
mod announcement_drafts;
mod announcements;
pub mod auth;
mod bookings;
mod catalog_error;
mod coach_acceptance_changes;
mod coach_acceptances;
mod coach_bookings;
mod coach_overviews;
mod coach_profiles;
mod coach_slots;
mod coach_videos;
mod coaches;
mod contract;
mod entitlement_error;
mod entitlements;
mod error;
mod favorite_coaches;
mod headquarters_booking_details;
mod headquarters_bookings;
mod headquarters_cancelled_bookings;
mod headquarters_live_lesson_sessions;
mod healthz;
mod incomplete_coach_profiles;
mod lesson_menu_publications;
mod lesson_menus;
mod lesson_stats;
mod lessons;
mod live_lesson_sessions;
mod low_rated_review_handlings;
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
mod overdue_live_lesson_sessions;
mod pending_video_reviews;
mod review_error;
mod review_replies;
mod reviews;
mod router_with;
mod scheduling_error;
mod shared_tokens;
mod slot_supply;
mod slot_withdrawals;
mod slots;
mod student_activity;
mod students;
mod subscription_plans;
mod subscriptions;
pub mod version;
mod videos;

pub use accounts::accounts_router;
pub use announcement_drafts::announcement_drafts_router;
pub use announcements::announcements_router;
pub use auth::{AuthenticatedAccount, require_role};
pub use bookings::bookings_router;
pub use coach_acceptances::coach_acceptance_router;
pub use coach_bookings::coach_bookings_router;
pub use coach_overviews::coach_overview_router;
pub use coach_profiles::coach_profile_router;
pub use coach_slots::coach_slots_router;
pub use coach_videos::coach_videos_router;
pub use coaches::coaches_router;
pub use entitlements::entitlements_router;
pub use error::ApiError;
pub use favorite_coaches::favorite_coaches_router;
pub use headquarters_booking_details::headquarters_booking_detail_router;
pub use headquarters_bookings::headquarters_bookings_router;
pub use headquarters_cancelled_bookings::headquarters_cancelled_bookings_router;
pub use headquarters_live_lesson_sessions::headquarters_live_lesson_sessions_router;
pub use healthz::healthz_router;
pub use incomplete_coach_profiles::incomplete_coach_profiles_router;
pub use lesson_menu_publications::lesson_menu_publication_router;
pub use lesson_menus::catalog_router;
pub use lesson_stats::lesson_stats_router;
pub use lessons::lessons_router;
pub use live_lesson_sessions::live_lesson_sessions_router;
pub use low_rated_review_handlings::low_rated_review_handlings_router;
pub use low_rated_reviews::low_rated_reviews_router;
pub use me::me_router;
pub use messaging::messaging_router;
pub use my_bookings::my_bookings_router;
pub use my_lesson_completions::my_lesson_completions_router;
pub use my_profile::my_profile_router;
pub use my_videos::my_videos_router;
pub use notification_settings::notification_settings_router;
pub use notifications::notifications_router;
pub use overdue_live_lesson_sessions::overdue_live_lesson_sessions_router;
pub use pending_video_reviews::pending_video_reviews_router;
pub use review_replies::review_replies_router;
pub use reviews::reviews_router;
pub use router_with::router_with;
pub use slot_supply::slot_supply_router;
pub use slot_withdrawals::slot_withdrawal_router;
pub use slots::slots_router;
pub use student_activity::student_activity_router;
pub use students::students_router;
pub use subscription_plans::subscription_plans_router;
pub use subscriptions::subscriptions_router;
pub use videos::videos_router;
