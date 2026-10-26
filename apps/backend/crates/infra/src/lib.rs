//! インフラ層。application のポートを Postgres（T04 以降）とモックの外部連携で実装する。
//!
//! 依存してよいのは domain と application（docs/adr/0003-backend-layers.md）。

mod mock_token;

pub use mock_token::MockTokenService;

/// `migrations/` のマイグレーション（コンパイル時に埋め込む）。
///
/// 変更は expand → 移行 → contract の 3 段で行い、contract は人間承認（docs/adr/0004-schema-expand-contract.md）。
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();
