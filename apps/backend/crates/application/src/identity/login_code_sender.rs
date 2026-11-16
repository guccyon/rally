use std::future::Future;

use domain::identity::{Email, LoginCode};

use crate::Unavailable;

/// ログインの確認コードをメールで送るポート（Gen1 はモック。実装は infra の `MockLoginCodeMailer` と、テスト用の
/// [`crate::in_memory::RecordingLoginCodeSender`]）。
pub trait LoginCodeSender: Send + Sync {
    /// `email` に `code` を送る。
    fn send(
        &self,
        email: &Email,
        code: &LoginCode,
    ) -> impl Future<Output = Result<(), Unavailable>> + Send;
}
