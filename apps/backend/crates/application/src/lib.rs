//! アプリケーション層。ユースケースと、infra が実装するポート（trait）を置く。
//!
//! 依存してよいのは domain だけ（docs/adr/0003-backend-layers.md）。

pub mod analytics;
pub mod async_lesson;
pub mod catalog;
pub mod clock;
pub mod domain_event;
pub mod entitlement;
pub mod identity;
pub mod in_memory;
pub mod lesson;
pub mod live_lesson;
pub mod matching;
pub mod messaging;
pub mod notification;
pub mod profile;
pub mod review;
pub mod scheduling;
mod unavailable;
mod uuid_text;

pub use unavailable::Unavailable;
