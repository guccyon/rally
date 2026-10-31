//! インフラ層。application のポートを Postgres とモックの外部連携（トークン・境界間イベント・通話基盤・動画の保存先・AI の一次フィードバック・プッシュ通知・行動トラッキング）で実装する。
//!
//! 依存してよいのは domain と application（docs/adr/0003-backend-layers.md）。

mod analytics_common_attributes;
mod analytics_destination;
pub mod analytics_event_payload;
pub mod catalog_values;
pub mod domain_event_payload;
pub mod entitlement_values;
pub mod identity_values;
mod in_memory_domain_event_bus;
mod logging_domain_event_publisher;
pub mod messaging_values;
mod mock_ai_feedback_generator;
mod mock_alt_video_call_provider;
mod mock_push_sender;
mod mock_token;
mod mock_video_call_provider;
mod mock_video_storage;
pub mod notification_values;
mod object_store_analytics_sink;
mod pg_account_repository;
mod pg_announcement_repository;
mod pg_booking_repository;
mod pg_entitlement_repository;
mod pg_favorite_coach_repository;
mod pg_lesson_completion_repository;
mod pg_lesson_menu_repository;
mod pg_lesson_review_repository;
mod pg_live_lesson_session_repository;
mod pg_message_thread_repository;
mod pg_notification_repository;
mod pg_notification_settings_repository;
mod pg_slot_repository;
mod pg_support;
mod pg_video_repository;
pub mod scheduling_values;
mod unknown_video_call_provider;
mod video_call_backend;

pub use analytics_common_attributes::AnalyticsCommonAttributes;
pub use analytics_destination::AnalyticsDestination;
pub use in_memory_domain_event_bus::InMemoryDomainEventBus;
pub use logging_domain_event_publisher::LoggingDomainEventPublisher;
pub use mock_ai_feedback_generator::MockAiFeedbackGenerator;
pub use mock_alt_video_call_provider::MockAltVideoCallProvider;
pub use mock_push_sender::MockPushSender;
pub use mock_token::MockTokenService;
pub use mock_video_call_provider::MockVideoCallProvider;
pub use mock_video_storage::MockVideoStorage;
pub use object_store_analytics_sink::ObjectStoreAnalyticsSink;
pub use pg_account_repository::PgAccountRepository;
pub use pg_announcement_repository::PgAnnouncementRepository;
pub use pg_booking_repository::PgBookingRepository;
pub use pg_entitlement_repository::PgEntitlementRepository;
pub use pg_favorite_coach_repository::PgFavoriteCoachRepository;
pub use pg_lesson_completion_repository::PgLessonCompletionRepository;
pub use pg_lesson_menu_repository::PgLessonMenuRepository;
pub use pg_lesson_review_repository::PgLessonReviewRepository;
pub use pg_live_lesson_session_repository::PgLiveLessonSessionRepository;
pub use pg_message_thread_repository::PgMessageThreadRepository;
pub use pg_notification_repository::PgNotificationRepository;
pub use pg_notification_settings_repository::PgNotificationSettingsRepository;
pub use pg_slot_repository::PgSlotRepository;
pub use pg_video_repository::PgVideoRepository;
pub use unknown_video_call_provider::UnknownVideoCallProvider;
pub use video_call_backend::VideoCallBackend;

/// `migrations/` のマイグレーション（コンパイル時に埋め込む）。
///
/// 変更は expand → 移行 → contract の 3 段で行い、contract は人間承認（docs/adr/0004-schema-expand-contract.md）。
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

/// 起動時に `pool` へ [`MIGRATOR`] のマイグレーションを当てる（当て済みのものは飛ばす）。
///
/// DB に当たっていて [`MIGRATOR`] が知らないマイグレーション（新しい版が当てたもの）があっても止めない
/// （sqlx の `set_ignore_missing(true)`）。Rollback は前の版のイメージに戻すだけで行うので、新しい版が当てた
/// expand・移行を DB に残したまま前の版が起動できなければならない。expand・移行は古い版がそのまま動く変更に
/// 限っている（docs/adr/0004-schema-expand-contract.md の 2）ので、知らない版を飛ばしても前の版は動く。
/// 当て済みのマイグレーションが書き換えられていれば（チェックサムの不一致）今までどおり止める。
pub async fn run_migrations(pool: &sqlx::PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    let mut migrator = sqlx::migrate::Migrator::with_migrations(MIGRATOR.iter().cloned().collect());
    migrator.set_ignore_missing(true);
    migrator.run(pool).await
}
