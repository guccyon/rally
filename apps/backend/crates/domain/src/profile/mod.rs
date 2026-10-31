//! profile: プロフィール（specs/context-map.yaml）。
//!
//! 相手に見せる本人の情報。今は表示名（[`DisplayName`]）とコーチの自己紹介文（[`CoachBio`]）。
//! アカウント（identity の [`crate::identity::Account`]）は表示名をこの型で持つ（表示名の移動は後の段）。自己紹介文は
//! identity の型には持たせず、profile のポート（application の `profile::CoachProfileRepository`）で読み書きする
//! （docs/adr/0009-subdomain-types.md）。

mod coach_bio;
mod display_name;

pub use coach_bio::CoachBio;
pub use display_name::DisplayName;
