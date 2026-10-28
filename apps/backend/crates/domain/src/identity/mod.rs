//! identity: アカウント・権限（specs/context-map.yaml）。
//!
//! 生徒（glossary の student）・コーチ（coach）・本部のアカウントを、役割（[`Role`]）と
//! 役割ごとの属性（[`Profile`]）で表す。

mod account;
mod account_id;
mod coach;
mod coach_bio;
mod display_name;
mod email;
mod profile;
mod role;
mod sport;
mod student;

pub use account::Account;
pub use account_id::AccountId;
pub use coach::Coach;
pub use coach_bio::CoachBio;
pub use display_name::DisplayName;
pub use email::Email;
pub use profile::Profile;
pub use role::Role;
pub use sport::Sport;
pub use student::Student;
