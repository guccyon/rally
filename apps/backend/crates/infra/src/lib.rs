//! インフラ層。application のポートを Postgres とモックの外部連携（トークン・境界間イベント・通話基盤・動画の保存先・行動トラッキング）で実装する。
//!
//! 依存してよいのは domain と application（docs/adr/0003-backend-layers.md）。

mod analytics_common_attributes;
mod analytics_destination;
pub mod analytics_event_payload;
pub mod domain_event_payload;
pub mod entitlement_values;
pub mod identity_values;
mod in_memory_domain_event_bus;
mod logging_domain_event_publisher;
pub mod messaging_values;
mod mock_token;
mod mock_video_call_provider;
mod mock_video_storage;
mod object_store_analytics_sink;
mod pg_account_repository;
mod pg_booking_repository;
mod pg_entitlement_repository;
mod pg_lesson_completion_repository;
mod pg_live_lesson_session_repository;
mod pg_message_thread_repository;
mod pg_slot_repository;
mod pg_support;
mod pg_video_repository;
pub mod scheduling_values;

pub use analytics_common_attributes::AnalyticsCommonAttributes;
pub use analytics_destination::AnalyticsDestination;
pub use in_memory_domain_event_bus::InMemoryDomainEventBus;
pub use logging_domain_event_publisher::LoggingDomainEventPublisher;
pub use mock_token::MockTokenService;
pub use mock_video_call_provider::MockVideoCallProvider;
pub use mock_video_storage::MockVideoStorage;
pub use object_store_analytics_sink::ObjectStoreAnalyticsSink;
pub use pg_account_repository::PgAccountRepository;
pub use pg_booking_repository::PgBookingRepository;
pub use pg_entitlement_repository::PgEntitlementRepository;
pub use pg_lesson_completion_repository::PgLessonCompletionRepository;
pub use pg_live_lesson_session_repository::PgLiveLessonSessionRepository;
pub use pg_message_thread_repository::PgMessageThreadRepository;
pub use pg_slot_repository::PgSlotRepository;
pub use pg_video_repository::PgVideoRepository;

/// `migrations/` のマイグレーション（コンパイル時に埋め込む）。
///
/// 変更は expand → 移行 → contract の 3 段で行い、contract は人間承認（docs/adr/0004-schema-expand-contract.md）。
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();
