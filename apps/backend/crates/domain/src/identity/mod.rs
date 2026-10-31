//! identity: アカウント・権限（specs/context-map.yaml）。
//!
//! 生徒（glossary の student）・コーチ（coach）・本部のアカウントを、役割（[`Role`]）と
//! 役割ごとの属性（[`RoleAttributes`]）で表す。表示名・自己紹介文は profile（[`crate::profile`]）の型。

mod account;
mod account_id;
mod coach;
mod email;
mod role;
mod role_attributes;
mod student;

pub use account::Account;
pub use account_id::AccountId;
pub use coach::Coach;
pub use email::Email;
pub use role::Role;
pub use role_attributes::RoleAttributes;
pub use student::Student;
