use std::future::Future;

use domain::identity::{Email, LoginCodeChallenge};

use crate::Unavailable;

/// ログインの確認コードの保存先のポート（ADR 0012）。メールアドレスごと（大文字小文字を区別しない。
/// `Email::normalized`）に 1 つを持つ。実装は infra（Postgres の `identity.login_codes`。アカウントの保存先が兼ねる）と、
/// [`crate::in_memory::InMemoryAccountRepository`]。
///
/// 間違いの数え上げと使い終わりは、読んだときと同じコード（同じメールアドレスで同じ時刻に送ったもの）にだけ効かせる。
/// 間に送り直されたら、前のコードの数え上げで新しいコードを変えない。
pub trait LoginCodeRepository: Send + Sync {
    /// `email` に送ったコード。なければ `None`。
    fn find_login_code(
        &self,
        email: &Email,
    ) -> impl Future<Output = Result<Option<LoginCodeChallenge>, Unavailable>> + Send;

    /// コードを保存する。同じメールアドレスのコードがあれば置き換える（前のコードは使えなくなる）。
    fn save_login_code(
        &self,
        challenge: LoginCodeChallenge,
    ) -> impl Future<Output = Result<(), Unavailable>> + Send;

    /// `challenge` の間違いの回数に 1 を足す（読んでから足すまでの間の、ほかの間違いも数え漏らさない）。
    fn record_failed_login_code_attempt(
        &self,
        challenge: &LoginCodeChallenge,
    ) -> impl Future<Output = Result<(), Unavailable>> + Send;

    /// `challenge` を使い終わりにして消す。消せたら `true`、すでに使われた・置き換わったなら `false`
    /// （同じコードで 2 回ログインさせない）。
    fn consume_login_code(
        &self,
        challenge: &LoginCodeChallenge,
    ) -> impl Future<Output = Result<bool, Unavailable>> + Send;
}
