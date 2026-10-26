//! インフラ層。application のポートを Postgres（T04 以降）とモックの外部連携で実装する。
//!
//! 依存してよいのは domain と application（docs/adr/0003-backend-layers.md）。

pub mod domain_event_payload;
pub mod identity_values;
mod in_memory_domain_event_bus;
mod logging_domain_event_publisher;
mod mock_token;
mod pg_slot_repository;
pub mod scheduling_values;

pub use in_memory_domain_event_bus::InMemoryDomainEventBus;
pub use logging_domain_event_publisher::LoggingDomainEventPublisher;
pub use mock_token::MockTokenService;
pub use pg_slot_repository::PgSlotRepository;

/// `migrations/` のマイグレーション（コンパイル時に埋め込む）。
///
/// 変更は expand → 移行 → contract の 3 段で行い、contract は人間承認（docs/adr/0004-schema-expand-contract.md）。
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();
