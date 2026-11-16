//! identity: アカウント・権限（specs/context-map.yaml）。
//!
//! 生徒（glossary の student）・コーチ（coach）・本部のアカウントを、役割（[`Role`]）と
//! 役割ごとの属性（[`RoleAttributes`]）で表す。生徒のログインはメールで送る確認コード（[`LoginCode`]・
//! [`LoginCodeChallenge`]。docs/adr/0012-student-login-one-time-code.md）。表示名・自己紹介文は profile（[`crate::profile`]）の型。

mod account;
mod account_id;
mod coach;
mod email;
mod login_code;
mod login_code_challenge;
mod login_code_verification;
mod role;
mod role_attributes;
mod student;

pub use account::Account;
pub use account_id::AccountId;
pub use coach::Coach;
pub use email::Email;
pub use login_code::LoginCode;
pub use login_code_challenge::LoginCodeChallenge;
pub use login_code_verification::LoginCodeVerification;
pub use role::Role;
pub use role_attributes::RoleAttributes;
pub use student::Student;
