use std::future::Future;

use domain::{
    identity::{Account, AccountId},
    profile::DisplayName,
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

    /// 生徒のアカウントをすべて、登録の新しい順（同じ登録時刻は ID の順）で返す。
    fn list_students(&self) -> impl Future<Output = Result<Vec<Account>, Unavailable>> + Send;

    /// 生徒 `student_id` の表示名を `display_name` に置き換え、置き換えたあとのアカウントを返す。生徒のアカウントが
    /// なければ（ID のアカウントがないか、生徒でなければ）何も変えずに `None`。
    fn update_student_display_name(
        &self,
        student_id: AccountId,
        display_name: DisplayName,
    ) -> impl Future<Output = Result<Option<Account>, Unavailable>> + Send;
}

#[derive(Debug)]
pub enum InsertAccountError {
    /// 同じメールアドレスのアカウントがすでにある
    EmailAlreadyRegistered,
    /// 保存先の障害
    Unavailable(Unavailable),
}
