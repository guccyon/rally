use super::AccountId;

/// 確認コードを確かめた結果（[`super::LoginCodeChallenge::verify`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginCodeVerification {
    /// コードが合った。`account_id` の生徒としてログインしてよい（コードはこれで使えなくなる）
    Accepted { account_id: AccountId },
    /// コードが違う（生徒のアカウントのないメールアドレスへの確かめも、ここに入る）。間違いの回数に 1 を足す
    Mismatched,
    /// 送ってから有効期限（[`super::LoginCodeChallenge::EXPIRES_IN`]）が過ぎた
    Expired,
    /// 間違いが上限（[`super::LoginCodeChallenge::MAX_FAILED_ATTEMPTS`]）に達していて、このコードはもう確かめない
    TooManyAttempts,
}
