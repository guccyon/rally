use std::time::{Duration, SystemTime};

use super::{AccountId, Email, LoginCode, LoginCodeVerification};

/// メールアドレス 1 つに送った（送ったことにした）ログインの確認コードと、その間違いの回数（ADR 0012）。
///
/// メールアドレスごとに 1 つで、送り直すと前のものに置き換わる。生徒のアカウントのないメールアドレスにも、送らないコードで
/// 同じように作る（`account_id` が `None`）。送り直しの待ち・期限・間違いの上限が登録の有無で変わらず、応答から登録の有無が
/// 分からないようにするため。`None` のものはコードが合っても [`LoginCodeVerification::Mismatched`] にする。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginCodeChallenge {
    email: Email,
    account_id: Option<AccountId>,
    code: LoginCode,
    issued_at: SystemTime,
    failed_attempts: u32,
}

impl LoginCodeChallenge {
    /// 有効期限（送ってから 10 分）。
    pub const EXPIRES_IN: Duration = Duration::from_secs(10 * 60);

    /// 同じメールアドレスへ送り直せるまでの間（60 秒）。
    pub const RESEND_INTERVAL: Duration = Duration::from_secs(60);

    /// 1 つのコードで間違えてよい回数。この回数だけ間違えると、そのコードはもう確かめない。
    pub const MAX_FAILED_ATTEMPTS: u32 = 5;

    /// `now` に送るコード。間違いの回数は 0。
    pub fn issue(
        email: Email,
        account_id: Option<AccountId>,
        code: LoginCode,
        now: SystemTime,
    ) -> Self {
        Self::restore(email, account_id, code, now, 0)
    }

    /// 保存した値から戻す。
    pub fn restore(
        email: Email,
        account_id: Option<AccountId>,
        code: LoginCode,
        issued_at: SystemTime,
        failed_attempts: u32,
    ) -> Self {
        Self {
            email,
            account_id,
            code,
            issued_at,
            failed_attempts,
        }
    }

    pub fn email(&self) -> &Email {
        &self.email
    }

    /// コードでログインする生徒。生徒のアカウントのないメールアドレスなら `None`（コードは送っていない）。
    pub const fn account_id(&self) -> Option<AccountId> {
        self.account_id
    }

    pub fn code(&self) -> &LoginCode {
        &self.code
    }

    pub const fn issued_at(&self) -> SystemTime {
        self.issued_at
    }

    pub const fn failed_attempts(&self) -> u32 {
        self.failed_attempts
    }

    /// 有効期限の時刻（この時刻からは期限切れ）。
    pub fn expires_at(&self) -> SystemTime {
        self.issued_at + Self::EXPIRES_IN
    }

    /// `now` に送り直すなら、あと何秒待つか。待たずに送り直せるなら `None`。
    pub fn resend_wait(&self, now: SystemTime) -> Option<Duration> {
        let resendable_at = self.issued_at + Self::RESEND_INTERVAL;
        resendable_at
            .duration_since(now)
            .ok()
            .filter(|wait| !wait.is_zero())
    }

    /// `now` に入力された `code` を確かめる。上限・期限・コードの順に見る（上限に達したコードは期限が切れても
    /// [`LoginCodeVerification::TooManyAttempts`]）。間違いの回数は変えない（足すのは保存先）。
    pub fn verify(&self, code: &LoginCode, now: SystemTime) -> LoginCodeVerification {
        if self.failed_attempts >= Self::MAX_FAILED_ATTEMPTS {
            return LoginCodeVerification::TooManyAttempts;
        }
        if now >= self.expires_at() {
            return LoginCodeVerification::Expired;
        }
        match self.account_id {
            Some(account_id) if self.code.matches(code) => {
                LoginCodeVerification::Accepted { account_id }
            }
            _ => LoginCodeVerification::Mismatched,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ISSUED: Duration = Duration::from_secs(1_793_000_000);

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + ISSUED + Duration::from_secs(secs)
    }

    fn code(s: &str) -> LoginCode {
        LoginCode::new(s).unwrap()
    }

    fn challenge(account_id: Option<AccountId>, failed_attempts: u32) -> LoginCodeChallenge {
        LoginCodeChallenge::restore(
            Email::new("taro@example.com").unwrap(),
            account_id,
            code("123456"),
            at(0),
            failed_attempts,
        )
    }

    const STUDENT: AccountId = AccountId::from_u128(7);

    #[test]
    fn issued_challenge_starts_without_failures() {
        let issued = LoginCodeChallenge::issue(
            Email::new("taro@example.com").unwrap(),
            Some(STUDENT),
            code("123456"),
            at(0),
        );
        assert_eq!(issued, challenge(Some(STUDENT), 0));
        assert_eq!(issued.expires_at(), at(600));
    }

    #[test]
    fn the_right_code_within_ten_minutes_is_accepted() {
        let challenge = challenge(Some(STUDENT), 4);
        for now in [at(0), at(599)] {
            assert_eq!(
                challenge.verify(&code("123456"), now),
                LoginCodeVerification::Accepted {
                    account_id: STUDENT
                }
            );
        }
    }

    #[test]
    fn a_wrong_code_is_mismatched() {
        assert_eq!(
            challenge(Some(STUDENT), 0).verify(&code("654321"), at(1)),
            LoginCodeVerification::Mismatched
        );
    }

    #[test]
    fn without_a_student_account_even_the_same_code_is_mismatched() {
        assert_eq!(
            challenge(None, 0).verify(&code("123456"), at(1)),
            LoginCodeVerification::Mismatched
        );
    }

    #[test]
    fn from_ten_minutes_on_the_code_is_expired() {
        for account_id in [Some(STUDENT), None] {
            for input in ["123456", "654321"] {
                assert_eq!(
                    challenge(account_id, 0).verify(&code(input), at(600)),
                    LoginCodeVerification::Expired
                );
            }
        }
    }

    #[test]
    fn after_five_failures_the_code_is_no_longer_checked() {
        for now in [at(1), at(601)] {
            assert_eq!(
                challenge(Some(STUDENT), 5).verify(&code("123456"), now),
                LoginCodeVerification::TooManyAttempts
            );
        }
    }

    #[test]
    fn resending_waits_sixty_seconds_from_the_last_issue() {
        let challenge = challenge(Some(STUDENT), 0);
        assert_eq!(challenge.resend_wait(at(0)), Some(Duration::from_secs(60)));
        assert_eq!(challenge.resend_wait(at(59)), Some(Duration::from_secs(1)));
        assert_eq!(challenge.resend_wait(at(60)), None);
        assert_eq!(challenge.resend_wait(at(3600)), None);
    }
}
