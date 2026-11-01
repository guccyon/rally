//! profile: プロフィール（specs/context-map.yaml）。
//!
//! 相手に見せる本人の情報。今は表示名（[`DisplayName`]）とコーチの自己紹介文（[`CoachBio`]）。表示名で探すときの検索語は
//! [`DisplayNameQuery`]。コーチのプロフィールの空にできる項目は [`CoachProfileField`]、そのうち足りない（空の）項目の判定は
//! [`MissingCoachProfileFields`]。本部がコーチを競技で絞る条件は [`CoachSportFilter`]。
//! アカウント（identity の [`crate::identity::Account`]）は表示名をこの型で持つ（表示名の移動は後の段）。自己紹介文は
//! identity の型には持たせず、profile のポート（application の `profile::CoachProfileRepository`）で読み書きする
//! （docs/adr/0009-subdomain-types.md）。

mod coach_bio;
mod coach_profile_field;
mod coach_sport_filter;
mod display_name;
mod display_name_query;
mod missing_coach_profile_fields;

pub use coach_bio::CoachBio;
pub use coach_profile_field::CoachProfileField;
pub use coach_sport_filter::CoachSportFilter;
pub use display_name::DisplayName;
pub use display_name_query::DisplayNameQuery;
pub use missing_coach_profile_fields::MissingCoachProfileFields;
