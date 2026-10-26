//! identity: アカウント・権限（specs/context-map.yaml）。
//!
//! 生徒（glossary の student）・コーチ（coach）・本部のアカウントを、役割（[`Role`]）と
//! 役割ごとの属性（[`Profile`]）で表す。DB の対応は `identity.accounts`（docs/adr/0004）。

mod account;
mod account_id;
mod display_name;
mod email;
mod sport;

pub use account::{Account, Coach, Profile, Role, Student};
pub use account_id::AccountId;
pub use display_name::DisplayName;
pub use email::Email;
pub use sport::Sport;
