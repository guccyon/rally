//! アプリケーション層。ユースケースと、infra が実装するポート（trait）を置く。
//!
//! 依存してよいのは domain だけ（docs/adr/0003-backend-layers.md）。

pub mod analytics;
pub mod clock;
pub mod identity;
pub mod in_memory;
pub mod scheduling;
mod unavailable;
mod uuid_text;

pub use unavailable::Unavailable;
