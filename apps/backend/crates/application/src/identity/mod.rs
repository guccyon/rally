//! identity のユースケースとポート（登録・本人のアカウントの照会・認証）と、他のコンテキストにコーチを渡すアダプタ
//! （[`IdentityCoachLookup`]）。表示名・自己紹介文を扱うユースケースは [`crate::profile`]。

pub mod account_id_uuid;
mod get_account;
mod identity_coach_lookup;
mod register_account;
mod repository;
mod token;

pub use get_account::GetAccount;
pub use identity_coach_lookup::IdentityCoachLookup;
pub use register_account::{RegisterAccount, RegisterAccountCommand, RegisterAccountError};
pub use repository::{AccountRepository, InsertAccountError};
pub use token::{AccessToken, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier};
