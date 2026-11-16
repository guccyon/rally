use std::time::Duration;

use domain::{
    DomainError,
    identity::{Account, Email, LoginCode, LoginCodeVerification},
};

use super::{AccountRepository, LoginCodeRepository};
use crate::{Unavailable, clock::Clock};

/// 生徒のログインの入力（検証前の値）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateSessionCommand {
    pub email: String,
    pub code: String,
}

#[derive(Debug)]
pub enum CreateSessionError {
    /// メールアドレス・コードの形が不正
    Invalid(DomainError),
    /// コードが違う・そのメールアドレスに送ったコードがない（生徒のアカウントのないメールアドレスも、ここに入る）
    CodeInvalid,
    /// 送ってから有効期限が過ぎた
    CodeExpired,
    /// そのコードで間違いが上限に達した。`retry_after` は新しいコードを送れるまでの時間（もう送れるなら 0）
    TooManyAttempts { retry_after: Duration },
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<DomainError> for CreateSessionError {
    fn from(error: DomainError) -> Self {
        Self::Invalid(error)
    }
}

impl From<Unavailable> for CreateSessionError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}

/// 生徒のログインのユースケース（student-app の `createSession`。ADR 0012）。
///
/// `requestLoginCode` で送ったコードを確かめ、合えばコードを使い終わりにしてその生徒のアカウントを返す（トークンの発行は
/// 境界（interface）が `registerAccount` と同じく行う）。違えば間違いの回数に 1 を足す。行動イベントは発火しない。
pub struct CreateSession<R, C> {
    accounts: R,
    clock: C,
}

impl<R, C> CreateSession<R, C>
where
    R: AccountRepository + LoginCodeRepository,
    C: Clock,
{
    pub fn new(accounts: R, clock: C) -> Self {
        Self { accounts, clock }
    }

    pub async fn execute(
        &self,
        command: CreateSessionCommand,
    ) -> Result<Account, CreateSessionError> {
        let email = Email::new(command.email)?;
        let code = LoginCode::new(command.code)?;
        let now = self.clock.now();

        let Some(challenge) = self.accounts.find_login_code(&email).await? else {
            return Err(CreateSessionError::CodeInvalid);
        };
        match challenge.verify(&code, now) {
            LoginCodeVerification::Accepted { account_id } => {
                if !self.accounts.consume_login_code(&challenge).await? {
                    // 同時に確かめた別の要求が先に使った・間に送り直された
                    return Err(CreateSessionError::CodeInvalid);
                }
                self.accounts
                    .find(account_id)
                    .await?
                    .filter(|account| account.as_student().is_some())
                    .ok_or(CreateSessionError::CodeInvalid)
            }
            LoginCodeVerification::Mismatched => {
                self.accounts
                    .record_failed_login_code_attempt(&challenge)
                    .await?;
                Err(CreateSessionError::CodeInvalid)
            }
            LoginCodeVerification::Expired => Err(CreateSessionError::CodeExpired),
            LoginCodeVerification::TooManyAttempts => Err(CreateSessionError::TooManyAttempts {
                retry_after: challenge.resend_wait(now).unwrap_or_default(),
            }),
        }
    }

    pub fn accounts(&self) -> &R {
        &self.accounts
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::{
        catalog::Sport,
        identity::{AccountId, LoginCodeChallenge, Role},
        profile::DisplayName,
    };

    use super::*;
    use crate::{
        identity::RequestLoginCode,
        in_memory::{InMemoryAccountRepository, ManualClock, RecordingLoginCodeSender},
    };

    const NOW: Duration = Duration::from_secs(1_793_000_000);
    const STUDENT: AccountId = AccountId::from_u128(1);

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + NOW + Duration::from_secs(secs)
    }

    struct Fixture {
        request: RequestLoginCode<InMemoryAccountRepository, RecordingLoginCodeSender, ManualClock>,
        create: CreateSession<InMemoryAccountRepository, ManualClock>,
        sender: RecordingLoginCodeSender,
        clock: ManualClock,
    }

    async fn fixture() -> Fixture {
        let accounts = InMemoryAccountRepository::default();
        for (id, email, role) in [
            (1, "taro@example.com", Role::Student),
            (2, "coach@example.com", Role::Coach),
        ] {
            accounts
                .insert(
                    Account::new(
                        AccountId::from_u128(id),
                        Email::new(email).unwrap(),
                        DisplayName::new("なまえ").unwrap(),
                        role,
                        [Sport::Tennis],
                        SystemTime::UNIX_EPOCH,
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
        }
        let sender = RecordingLoginCodeSender::default();
        let clock = ManualClock::new(at(0));
        Fixture {
            request: RequestLoginCode::new(accounts.clone(), sender.clone(), clock.clone()),
            create: CreateSession::new(accounts, clock.clone()),
            sender,
            clock,
        }
    }

    impl Fixture {
        async fn send(&self, email: &str) -> String {
            self.request.execute(email.to_string()).await.unwrap();
            self.sender
                .last_code_to(email)
                .map(|code| code.as_str().to_string())
                .unwrap_or_else(|| "000000".to_string())
        }

        async fn login(&self, email: &str, code: &str) -> Result<Account, CreateSessionError> {
            self.create
                .execute(CreateSessionCommand {
                    email: email.to_string(),
                    code: code.to_string(),
                })
                .await
        }
    }

    /// `code` と違う 6 桁のコード。
    fn other_than(code: &str) -> String {
        if code == "000000" {
            "111111".to_string()
        } else {
            "000000".to_string()
        }
    }

    #[tokio::test]
    async fn the_sent_code_logs_the_student_in_once() {
        let f = fixture().await;
        let code = f.send("taro@example.com").await;

        f.clock.set(at(599));
        let account = f.login("Taro@Example.com", &code).await.unwrap();
        assert_eq!(account.id(), STUDENT);

        let again = f.login("taro@example.com", &code).await;
        assert!(
            matches!(again, Err(CreateSessionError::CodeInvalid)),
            "{again:?}"
        );
    }

    #[tokio::test]
    async fn a_wrong_code_or_no_code_sent_is_code_invalid() {
        let f = fixture().await;
        let none = f.login("taro@example.com", "123456").await;
        assert!(
            matches!(none, Err(CreateSessionError::CodeInvalid)),
            "{none:?}"
        );

        let code = f.send("taro@example.com").await;
        let wrong = f.login("taro@example.com", &other_than(&code)).await;
        assert!(
            matches!(wrong, Err(CreateSessionError::CodeInvalid)),
            "{wrong:?}"
        );
        // 間違えても、上限までは正しいコードでログインできる
        assert!(f.login("taro@example.com", &code).await.is_ok());
    }

    #[tokio::test]
    async fn emails_without_a_student_account_are_code_invalid_like_a_wrong_code() {
        let f = fixture().await;
        for email in ["nobody@example.com", "coach@example.com"] {
            f.send(email).await;
            for code in ["000000", "123456", "999999"] {
                let result = f.login(email, code).await;
                assert!(
                    matches!(result, Err(CreateSessionError::CodeInvalid)),
                    "{email} {code}: {result:?}"
                );
            }
        }
    }

    #[tokio::test]
    async fn after_ten_minutes_the_code_is_expired() {
        let f = fixture().await;
        let code = f.send("taro@example.com").await;

        f.clock.set(at(600));
        let result = f.login("taro@example.com", &code).await;
        assert!(
            matches!(result, Err(CreateSessionError::CodeExpired)),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn five_mistakes_lock_the_code_until_a_new_one_is_sent() {
        for email in ["taro@example.com", "nobody@example.com"] {
            let f = fixture().await;
            let code = f.send(email).await;
            let wrong = other_than(&code);
            for _ in 0..LoginCodeChallenge::MAX_FAILED_ATTEMPTS {
                let result = f.login(email, &wrong).await;
                assert!(
                    matches!(result, Err(CreateSessionError::CodeInvalid)),
                    "{result:?}"
                );
            }

            f.clock.set(at(10));
            let locked = f.login(email, &code).await;
            assert!(
                matches!(
                    locked,
                    Err(CreateSessionError::TooManyAttempts { retry_after })
                        if retry_after == Duration::from_secs(50)
                ),
                "{email}: {locked:?}"
            );

            f.clock.set(at(60));
            let code = f.send(email).await;
            if email == "taro@example.com" {
                assert_eq!(f.login(email, &code).await.unwrap().id(), STUDENT);
            }
        }
    }

    #[tokio::test]
    async fn malformed_input_is_a_domain_error() {
        let f = fixture().await;
        for (email, code, expected) in [
            ("taro", "123456", DomainError::InvalidEmail),
            ("taro@example.com", "12345", DomainError::InvalidLoginCode),
            ("taro@example.com", "abcdef", DomainError::InvalidLoginCode),
        ] {
            let result = f.login(email, code).await;
            assert!(
                matches!(&result, Err(CreateSessionError::Invalid(e)) if *e == expected),
                "{email} {code}: {result:?}"
            );
        }
    }
}
