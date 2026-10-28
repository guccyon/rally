use domain::{
    DomainError,
    identity::{Account, Email, Profile, Role, Sport},
    profile::DisplayName,
};

use super::{AccountRepository, InsertAccountError};
use crate::{
    Unavailable,
    analytics::{AnalyticsEvent, AnalyticsSink},
    clock::Clock,
};

/// アカウント登録の入力（検証前の値）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterAccountCommand {
    pub email: String,
    pub display_name: String,
    pub role: Role,
    pub sports: Vec<Sport>,
}

#[derive(Debug)]
pub enum RegisterAccountError {
    /// 入力がドメインの不変条件を満たさない
    Invalid(DomainError),
    /// 同じメールアドレスのアカウントがすでにある
    EmailAlreadyRegistered,
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<DomainError> for RegisterAccountError {
    fn from(error: DomainError) -> Self {
        Self::Invalid(error)
    }
}

/// アカウント登録のユースケース。
///
/// 入力をドメインのモデルにし（不変条件の検証）、保存する。生徒の登録なら行動イベント
/// `account.registered` を 1 回発火する（events.yaml の actor は student で、コーチ・本部の登録に
/// 当たるイベントは定義されていない）。保存に失敗したら発火しない。
pub struct RegisterAccount<R, A, C> {
    accounts: R,
    analytics: A,
    clock: C,
}

impl<R, A, C> RegisterAccount<R, A, C>
where
    R: AccountRepository,
    A: AnalyticsSink,
    C: Clock,
{
    pub fn new(accounts: R, analytics: A, clock: C) -> Self {
        Self {
            accounts,
            analytics,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: RegisterAccountCommand,
    ) -> Result<Account, RegisterAccountError> {
        let email = Email::new(command.email)?;
        let display_name = DisplayName::new(command.display_name)?;
        let account = Account::new(
            self.accounts.next_id(),
            email,
            display_name,
            command.role,
            command.sports,
            self.clock.now(),
        )?;

        self.accounts
            .insert(account.clone())
            .await
            .map_err(|error| match error {
                InsertAccountError::EmailAlreadyRegistered => {
                    RegisterAccountError::EmailAlreadyRegistered
                }
                InsertAccountError::Unavailable(reason) => {
                    RegisterAccountError::Unavailable(reason)
                }
            })?;

        if let Profile::Student(student) = account.profile() {
            self.analytics
                .emit(AnalyticsEvent::AccountRegistered {
                    student_id: account.id(),
                    sport: student.sport(),
                    occurred_at: account.registered_at(),
                })
                .await;
        }

        Ok(account)
    }

    pub fn accounts(&self) -> &R {
        &self.accounts
    }

    pub fn analytics(&self) -> &A {
        &self.analytics
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryAccountRepository, RecordingAnalyticsSink};

    type UseCase = RegisterAccount<InMemoryAccountRepository, RecordingAnalyticsSink, FixedClock>;

    const NOW: Duration = Duration::from_secs(1_793_000_000);

    fn use_case() -> UseCase {
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            RecordingAnalyticsSink::default(),
            FixedClock(SystemTime::UNIX_EPOCH + NOW),
        )
    }

    fn student(email: &str, display_name: &str) -> RegisterAccountCommand {
        RegisterAccountCommand {
            email: email.to_string(),
            display_name: display_name.to_string(),
            role: Role::Student,
            sports: vec![Sport::Tennis],
        }
    }

    #[tokio::test]
    async fn registers_a_student_and_emits_account_registered_once() {
        let use_case = use_case();

        let account = use_case
            .execute(student("taro@example.com", "たろう"))
            .await
            .unwrap();

        assert_eq!(account.role(), Role::Student);
        assert_eq!(account.display_name().as_str(), "たろう");
        assert_eq!(account.registered_at(), SystemTime::UNIX_EPOCH + NOW);
        assert_eq!(use_case.accounts().all(), vec![account.clone()]);
        assert_eq!(
            use_case.analytics().events(),
            vec![AnalyticsEvent::AccountRegistered {
                student_id: account.id(),
                sport: Sport::Tennis,
                occurred_at: SystemTime::UNIX_EPOCH + NOW,
            }]
        );
        assert_eq!(
            use_case.analytics().events()[0].name(),
            "account.registered"
        );
    }

    #[tokio::test]
    async fn invalid_input_is_a_domain_error_and_nothing_is_saved_or_emitted() {
        let use_case = use_case();

        let cases = [
            (
                student("taro@example.com", " "),
                DomainError::EmptyDisplayName,
            ),
            (student("taro", "たろう"), DomainError::InvalidEmail),
            (
                RegisterAccountCommand {
                    sports: vec![],
                    ..student("taro@example.com", "たろう")
                },
                DomainError::RoleAttributeMismatch {
                    role: Role::Student,
                    sports: 0,
                },
            ),
            (
                RegisterAccountCommand {
                    role: Role::Coach,
                    sports: vec![],
                    ..student("taro@example.com", "たろう")
                },
                DomainError::RoleAttributeMismatch {
                    role: Role::Coach,
                    sports: 0,
                },
            ),
        ];
        for (command, expected) in cases {
            let result = use_case.execute(command).await;
            assert!(
                matches!(&result, Err(RegisterAccountError::Invalid(e)) if *e == expected),
                "{result:?}"
            );
        }

        assert!(use_case.accounts().all().is_empty());
        assert!(use_case.analytics().events().is_empty());
    }

    #[tokio::test]
    async fn duplicate_email_is_rejected_case_insensitively_without_emitting() {
        let use_case = use_case();
        use_case
            .execute(student("taro@example.com", "たろう"))
            .await
            .unwrap();

        let result = use_case
            .execute(student("TARO@example.com", "たろう 2"))
            .await;
        assert!(
            matches!(result, Err(RegisterAccountError::EmailAlreadyRegistered)),
            "{result:?}"
        );
        assert_eq!(use_case.accounts().all().len(), 1);
        assert_eq!(use_case.analytics().events().len(), 1);
    }

    #[tokio::test]
    async fn registering_a_coach_does_not_emit_account_registered() {
        let use_case = use_case();

        let account = use_case
            .execute(RegisterAccountCommand {
                email: "coach@example.com".to_string(),
                display_name: "コーチ".to_string(),
                role: Role::Coach,
                sports: vec![Sport::Tennis, Sport::Golf],
            })
            .await
            .unwrap();

        assert_eq!(account.as_coach().unwrap().sports().len(), 2);
        assert!(use_case.analytics().events().is_empty());
    }

    /// 保存に必ず失敗する保存先。
    struct BrokenRepository;

    impl AccountRepository for BrokenRepository {
        fn next_id(&self) -> domain::identity::AccountId {
            domain::identity::AccountId::from_u128(1)
        }

        async fn insert(&self, _account: Account) -> Result<(), InsertAccountError> {
            Err(InsertAccountError::Unavailable(Unavailable::new(
                "connection refused",
            )))
        }

        async fn find(
            &self,
            _id: domain::identity::AccountId,
        ) -> Result<Option<Account>, Unavailable> {
            Err(Unavailable::new("connection refused"))
        }

        async fn list_coaches(&self) -> Result<Vec<Account>, Unavailable> {
            Err(Unavailable::new("connection refused"))
        }

        async fn update_coach_bio(
            &self,
            _coach_id: domain::identity::AccountId,
            _bio: domain::profile::CoachBio,
        ) -> Result<Option<Account>, Unavailable> {
            Err(Unavailable::new("connection refused"))
        }
    }

    #[tokio::test]
    async fn storage_failure_is_unavailable_with_its_source_and_nothing_is_emitted() {
        let use_case = RegisterAccount::new(
            BrokenRepository,
            RecordingAnalyticsSink::default(),
            FixedClock(SystemTime::UNIX_EPOCH + NOW),
        );

        let result = use_case
            .execute(student("taro@example.com", "たろう"))
            .await;

        let Err(RegisterAccountError::Unavailable(error)) = result else {
            panic!("{result:?}");
        };
        assert_eq!(
            std::error::Error::source(&error).unwrap().to_string(),
            "connection refused"
        );
        assert!(use_case.analytics().events().is_empty());
    }
}
