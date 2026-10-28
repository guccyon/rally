//! profile: プロフィール（specs/context-map.yaml）。
//!
//! 相手に見せる本人の情報。今は表示名（[`DisplayName`]）とコーチの自己紹介文（[`CoachBio`]）。
//! アカウント（identity の [`crate::identity::Account`]）は表示名をこの型で持ち、コーチの属性
//! （[`crate::identity::Coach`]）は自己紹介文をこの型で持つ（docs/adr/0009-subdomain-types.md）。

mod coach_bio;
mod display_name;

pub use coach_bio::CoachBio;
pub use display_name::DisplayName;
