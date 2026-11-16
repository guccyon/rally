use std::time::Duration;

use domain::{
    DomainError,
    identity::{Email, LoginCode, LoginCodeChallenge},
};

use super::{AccountRepository, LoginCodeRepository, LoginCodeSender};
use crate::{Unavailable, clock::Clock};

#[derive(Debug)]
pub enum RequestLoginCodeError {
    /// メールアドレスの形が不正
    Invalid(DomainError),
    /// 前に送ってから [`LoginCodeChallenge::RESEND_INTERVAL`] たっていない。`retry_after` は送り直せるまでの時間
    TooManyRequests { retry_after: Duration },
    /// 保存先・送り先の障害
    Unavailable(Unavailable),
}

impl From<DomainError> for RequestLoginCodeError {
    fn from(error: DomainError) -> Self {
        Self::Invalid(error)
    }
}

impl From<Unavailable> for RequestLoginCodeError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}

/// 生徒のログインの確認コードを送るユースケース（student-app の `requestLoginCode`。ADR 0012）。
///
/// 6 桁のコードを乱数で作って保存し（前のコードは使えなくなる）、そのメールアドレスが生徒のアカウントのものなら
/// メールで送る。生徒のアカウントのないメールアドレス（登録がない・コーチや本部のもの）にも、送らないコードを同じように
/// 保存する。送り直しの待ち（60 秒）・期限・間違いの上限を登録の有無で変えず、応答から登録の有無と役割が分からないように
/// するため。行動イベントは発火しない。
pub struct RequestLoginCode<R, S, C> {
    accounts: R,
    sender: S,
    clock: C,
}

impl<R, S, C> RequestLoginCode<R, S, C>
where
    R: AccountRepository + LoginCodeRepository,
    S: LoginCodeSender,
    C: Clock,
{
    pub fn new(accounts: R, sender: S, clock: C) -> Self {
        Self {
            accounts,
            sender,
            clock,
        }
    }

    /// `email` に確認コードを送る（送ったことにする）。
    pub async fn execute(&self, email: String) -> Result<(), RequestLoginCodeError> {
        let email = Email::new(email)?;
        let now = self.clock.now();

        if let Some(previous) = self.accounts.find_login_code(&email).await?
            && let Some(retry_after) = previous.resend_wait(now)
        {
            return Err(RequestLoginCodeError::TooManyRequests { retry_after });
        }

        let student_id = self
            .accounts
            .find_by_email(&email)
            .await?
            .filter(|account| account.as_student().is_some())
            .map(|account| account.id());
        let code = LoginCode::from_number(rand::random_range(0..1_000_000));
        let challenge = LoginCodeChallenge::issue(email, student_id, code, now);
        self.accounts.save_login_code(challenge.clone()).await?;

        if student_id.is_some() {
            self.sender
                .send(challenge.email(), challenge.code())
                .await?;
        }
        Ok(())
    }

    pub fn accounts(&self) -> &R {
        &self.accounts
    }

    pub fn sender(&self) -> &S {
        &self.sender
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::{
        catalog::Sport,
        identity::{Account, AccountId, Role},
        profile::DisplayName,
    };

    use super::*;
    use crate::in_memory::{InMemoryAccountRepository, ManualClock, RecordingLoginCodeSender};

    const NOW: Duration = Duration::from_secs(1_793_000_000);

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + NOW + Duration::from_secs(secs)
    }

    async fn use_case()
    -> RequestLoginCode<InMemoryAccountRepository, RecordingLoginCodeSender, ManualClock> {
        let accounts = InMemoryAccountRepository::default();
        for (id, email, role, sports) in [
            (1, "taro@example.com", Role::Student, &[Sport::Tennis][..]),
            (2, "coach@example.com", Role::Coach, &[Sport::Tennis][..]),
            (3, "hq@example.com", Role::Headquarters, &[][..]),
        ] {
            accounts
                .insert(
                    Account::new(
                        AccountId::from_u128(id),
                        Email::new(email).unwrap(),
                        DisplayName::new("なまえ").unwrap(),
                        role,
                        sports.iter().copied(),
                        SystemTime::UNIX_EPOCH,
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
        }
        RequestLoginCode::new(
            accounts,
            RecordingLoginCodeSender::default(),
            ManualClock::new(at(0)),
        )
    }

    #[tokio::test]
    async fn sends_a_six_digit_code_to_a_student_and_keeps_it_for_ten_minutes() {
        let use_case = use_case().await;

        use_case
            .execute("TARO@example.com".to_string())
            .await
            .unwrap();

        let sent = use_case.sender().sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0.as_str(), "TARO@example.com");
        let saved = use_case
            .accounts()
            .find_login_code(&Email::new("taro@example.com").unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.code(), &sent[0].1);
        assert_eq!(saved.account_id(), Some(AccountId::from_u128(1)));
        assert_eq!(saved.issued_at(), at(0));
        assert_eq!(saved.expires_at(), at(600));
        assert_eq!(saved.failed_attempts(), 0);
    }

    #[tokio::test]
    async fn unknown_or_non_student_emails_succeed_alike_without_sending() {
        let use_case = use_case().await;

        for email in ["nobody@example.com", "coach@example.com", "hq@example.com"] {
            use_case.execute(email.to_string()).await.unwrap();
            let saved = use_case
                .accounts()
                .find_login_code(&Email::new(email).unwrap())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(saved.account_id(), None, "{email}");
        }
        assert!(use_case.sender().sent().is_empty());
    }

    #[tokio::test]
    async fn resending_within_sixty_seconds_is_too_many_requests_for_any_email() {
        for email in ["taro@example.com", "nobody@example.com"] {
            let use_case = use_case().await;
            use_case.execute(email.to_string()).await.unwrap();

            use_case.clock.set(at(59));
            let result = use_case.execute(email.to_uppercase()).await;
            assert!(
                matches!(
                    result,
                    Err(RequestLoginCodeError::TooManyRequests { retry_after })
                        if retry_after == Duration::from_secs(1)
                ),
                "{email}: {result:?}"
            );
        }
    }

    #[tokio::test]
    async fn resending_after_sixty_seconds_replaces_the_previous_code() {
        let use_case = use_case().await;
        use_case
            .execute("taro@example.com".to_string())
            .await
            .unwrap();

        use_case.clock.set(at(60));
        use_case
            .execute("taro@example.com".to_string())
            .await
            .unwrap();

        let sent = use_case.sender().sent();
        assert_eq!(sent.len(), 2);
        let saved = use_case
            .accounts()
            .find_login_code(&Email::new("taro@example.com").unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.code(), &sent[1].1);
        assert_eq!(saved.issued_at(), at(60));
    }

    #[tokio::test]
    async fn malformed_email_is_a_domain_error_and_nothing_is_saved() {
        let use_case = use_case().await;
        for email in ["", "taro", "ta ro@example.com"] {
            let result = use_case.execute(email.to_string()).await;
            assert!(
                matches!(
                    result,
                    Err(RequestLoginCodeError::Invalid(DomainError::InvalidEmail))
                ),
                "{email:?}: {result:?}"
            );
        }
        assert!(use_case.sender().sent().is_empty());
    }
}
