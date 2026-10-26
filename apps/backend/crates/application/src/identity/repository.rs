use std::future::Future;

use domain::identity::{Account, AccountId};

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertAccountError {
    /// 同じメールアドレスのアカウントがすでにある
    EmailAlreadyRegistered,
    /// 保存先の障害
    Unavailable(String),
}
