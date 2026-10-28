use std::future::Future;

use domain::{
    identity::{Account, AccountId},
    profile::CoachBio,
};

use crate::Unavailable;

/// アカウントの保存先のポート。実装は infra（Postgres の `identity.accounts`）と、
/// テスト用の [`crate::in_memory::InMemoryAccountRepository`]。
pub trait AccountRepository: Send + Sync {
    /// 新しいアカウントの ID を払い出す。
    fn next_id(&self) -> AccountId;

    /// アカウントを保存する。メールアドレスは大文字小文字を区別せず一意
    /// （`Email::normalized`。DB の `accounts_email_key`）で、重なれば
    /// [`InsertAccountError::EmailAlreadyRegistered`] を返す。
    fn insert(
        &self,
        account: Account,
    ) -> impl Future<Output = Result<(), InsertAccountError>> + Send;

    /// ID のアカウント。なければ `None`。
    fn find(
        &self,
        id: AccountId,
    ) -> impl Future<Output = Result<Option<Account>, Unavailable>> + Send;

    /// コーチのアカウントをすべて、表示名の順（Unicode のコードポイントの順。同じ表示名は ID の順）で返す。
    fn list_coaches(&self) -> impl Future<Output = Result<Vec<Account>, Unavailable>> + Send;

    /// コーチ `coach_id` の自己紹介文を `bio` に置き換え、置き換えたあとのアカウントを返す。コーチのアカウントが
    /// なければ（ID のアカウントがないか、コーチでなければ）何も変えずに `None`。
    fn update_coach_bio(
        &self,
        coach_id: AccountId,
        bio: CoachBio,
    ) -> impl Future<Output = Result<Option<Account>, Unavailable>> + Send;
}

#[derive(Debug)]
pub enum InsertAccountError {
    /// 同じメールアドレスのアカウントがすでにある
    EmailAlreadyRegistered,
    /// 保存先の障害
    Unavailable(Unavailable),
}
