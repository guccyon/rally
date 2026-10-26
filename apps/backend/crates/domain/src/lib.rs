//! ドメイン層。境界づけられたコンテキストのモデルと不変条件を置く。
//!
//! 純粋に保つ: I/O・非同期ランタイム・フレームワークに依存しない。
//! ワークスペースの他のクレートにも依存しない（docs/adr/0003-backend-layers.md）。

mod error;
pub mod identity;
pub mod scheduling;

pub use error::DomainError;
