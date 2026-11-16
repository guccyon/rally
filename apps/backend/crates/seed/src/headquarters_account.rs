use application::{
    analytics::AnalyticsSink,
    clock::Clock,
    identity::{AccountRepository, RegisterAccount, RegisterAccountCommand, RegisterAccountError},
};
use domain::identity::{AccountId, Email, Role};

use crate::HeadquartersAccountArgs;

/// `headquarters-account` が用意した本部のアカウント。`created` は今回登録したか（`false` なら同じメールアドレスの本部の
/// アカウントがすでにあり、何もしなかった）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadquartersAccount {
    pub account_id: AccountId,
    pub created: bool,
}

impl HeadquartersAccount {
    /// 本部のアカウントを登録のユースケース（`Role::Headquarters`）で 1 つ作る。同じメールアドレス（大文字・小文字を区別
    /// しない）の本部のアカウントがあれば何もせずそれを返す（表示名が違っても書き換えない）。そのメールアドレスを本部以外の
    /// アカウントが使っていればエラー（本部に変えない）。何度実行しても本部のアカウントは 1 つ。
    pub async fn ensure<R, A, C>(
        register: &RegisterAccount<R, A, C>,
        args: &HeadquartersAccountArgs,
    ) -> Result<Self, String>
    where
        R: AccountRepository,
        A: AnalyticsSink,
        C: Clock,
    {
        let registered = register
            .execute(RegisterAccountCommand {
                email: args.email.clone(),
                display_name: args.display_name.clone(),
                role: Role::Headquarters,
                sports: vec![],
            })
            .await;
        match registered {
            Ok(account) => Ok(Self {
                account_id: account.id(),
                created: true,
            }),
            Err(RegisterAccountError::EmailAlreadyRegistered) => {
                let email = Email::new(args.email.as_str()).map_err(|e| e.to_string())?;
                let account = register
                    .accounts()
                    .find_by_email(&email)
                    .await
                    .map_err(|e| format!("アカウントを読めない: {e}"))?
                    .ok_or_else(|| {
                        format!("メールアドレス {email} のアカウントが登録済みなのに読めない")
                    })?;
                if account.role() != Role::Headquarters {
                    return Err(format!(
                        "メールアドレス {email} は本部でないアカウント（{:?}）が使っている",
                        account.role()
                    ));
                }
                Ok(Self {
                    account_id: account.id(),
                    created: false,
                })
            }
            Err(RegisterAccountError::Invalid(e)) => Err(format!("入力が不正: {e}")),
            Err(RegisterAccountError::Unavailable(e)) => Err(format!("登録できない: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use application::in_memory::{FixedClock, InMemoryAccountRepository, RecordingAnalyticsSink};
    use domain::catalog::Sport;

    use super::*;

    type UseCase = RegisterAccount<InMemoryAccountRepository, RecordingAnalyticsSink, FixedClock>;

    fn use_case() -> UseCase {
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            RecordingAnalyticsSink::default(),
            FixedClock(SystemTime::UNIX_EPOCH),
        )
    }

    fn args(email: &str, display_name: &str) -> HeadquartersAccountArgs {
        HeadquartersAccountArgs {
            email: email.to_string(),
            display_name: display_name.to_string(),
        }
    }

    #[tokio::test]
    async fn second_run_with_the_same_email_does_nothing_and_returns_the_same_account() {
        let use_case = use_case();

        let first = HeadquartersAccount::ensure(&use_case, &args("hq@example.com", "本部"))
            .await
            .unwrap();
        let second = HeadquartersAccount::ensure(&use_case, &args("HQ@example.com", "別の名前"))
            .await
            .unwrap();

        assert!(first.created);
        assert_eq!(
            second,
            HeadquartersAccount {
                account_id: first.account_id,
                created: false,
            }
        );
        let accounts = use_case.accounts().all();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].role(), Role::Headquarters);
        assert_eq!(accounts[0].display_name().as_str(), "本部", "書き換えない");
        assert!(use_case.analytics().events().is_empty());
    }

    #[tokio::test]
    async fn email_of_a_non_headquarters_account_is_an_error() {
        let use_case = use_case();
        use_case
            .execute(RegisterAccountCommand {
                email: "coach@example.com".to_string(),
                display_name: "コーチ".to_string(),
                role: Role::Coach,
                sports: vec![Sport::Tennis],
            })
            .await
            .unwrap();

        let result =
            HeadquartersAccount::ensure(&use_case, &args("coach@example.com", "本部")).await;

        assert!(result.is_err(), "{result:?}");
        assert_eq!(use_case.accounts().all().len(), 1);
    }

    #[tokio::test]
    async fn invalid_input_is_an_error_and_nothing_is_saved() {
        let use_case = use_case();

        for args in [args("hq", "本部"), args("hq@example.com", " ")] {
            let result = HeadquartersAccount::ensure(&use_case, &args).await;
            assert!(result.is_err(), "{result:?}");
        }
        assert!(use_case.accounts().all().is_empty());
    }
}
