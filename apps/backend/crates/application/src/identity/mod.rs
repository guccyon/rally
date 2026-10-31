//! identity のユースケースとポート（登録・本人のアカウントの照会・本部が見る生徒の一覧・認証）。表示名・自己紹介文を扱うユースケースは
//! [`crate::profile`]。

pub mod account_id_uuid;
mod get_account;
mod list_students;
mod register_account;
mod repository;
mod student_contract;
mod student_contract_lookup;
mod student_last_booking_lookup;
mod student_overview;
mod token;

pub use get_account::GetAccount;
pub use list_students::ListStudents;
pub use register_account::{RegisterAccount, RegisterAccountCommand, RegisterAccountError};
pub use repository::{AccountRepository, InsertAccountError};
pub use student_contract::StudentContract;
pub use student_contract_lookup::StudentContractLookup;
pub use student_last_booking_lookup::StudentLastBookingLookup;
pub use student_overview::StudentOverview;
pub use token::{AccessToken, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier};
