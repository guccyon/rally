use domain::{
    DomainError,
    identity::{Account, AccountId},
    profile::DisplayName,
};

use crate::{Unavailable, identity::AccountRepository};

/// 生徒の表示名の変更の入力（検証前の値）。`student_id` は認証したアカウント（役割の確認は境界が行う）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateStudentProfileCommand {
    pub student_id: AccountId,
    pub display_name: String,
}

#[derive(Debug)]
pub enum UpdateStudentProfileError {
    /// 入力がドメインの不変条件を満たさない（表示名が空・長すぎる）
    Invalid(DomainError),
    /// `student_id` の生徒のアカウントがない
    StudentNotFound,
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<DomainError> for UpdateStudentProfileError {
    fn from(error: DomainError) -> Self {
        Self::Invalid(error)
    }
}

impl From<Unavailable> for UpdateStudentProfileError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}

/// 自分の表示名の変更のユースケース（student-app の `updateMyStudentProfile`）。表示名を置き換え、変えたあとの
/// アカウントを返す。表示名の規則は登録と同じ [`DisplayName`]。行動イベントも境界間イベントも発火しない。
pub struct UpdateStudentProfile<R> {
    accounts: R,
}

impl<R> UpdateStudentProfile<R>
where
    R: AccountRepository,
{
    pub fn new(accounts: R) -> Self {
        Self { accounts }
    }

    pub async fn execute(
        &self,
        command: UpdateStudentProfileCommand,
    ) -> Result<Account, UpdateStudentProfileError> {
        let display_name = DisplayName::new(command.display_name)?;
        self.accounts
            .update_student_display_name(command.student_id, display_name)
            .await?
            .ok_or(UpdateStudentProfileError::StudentNotFound)
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::{
        catalog::Sport,
        identity::{Email, Role},
    };

    use super::*;
    use crate::in_memory::InMemoryAccountRepository;

    const HEADQUARTERS: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(2);
    const STUDENT: AccountId = AccountId::from_u128(3);

    fn account(id: AccountId, role: Role, sports: &[Sport]) -> Account {
        Account::new(
            id,
            Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
            DisplayName::new("テスト").unwrap(),
            role,
            sports.iter().copied(),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    async fn accounts() -> InMemoryAccountRepository {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(HEADQUARTERS, Role::Headquarters, &[]),
            account(COACH, Role::Coach, &[Sport::Tennis]),
            account(STUDENT, Role::Student, &[Sport::Soccer]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        accounts
    }

    fn command(
        student_id: AccountId,
        display_name: impl Into<String>,
    ) -> UpdateStudentProfileCommand {
        UpdateStudentProfileCommand {
            student_id,
            display_name: display_name.into(),
        }
    }

    fn display_name_of(accounts: &InMemoryAccountRepository, id: AccountId) -> String {
        accounts
            .all()
            .into_iter()
            .find(|a| a.id() == id)
            .unwrap()
            .display_name()
            .as_str()
            .to_string()
    }

    #[tokio::test]
    async fn replaces_the_display_name_and_returns_the_updated_account() {
        let accounts = accounts().await;
        let before = accounts
            .all()
            .into_iter()
            .find(|a| a.id() == STUDENT)
            .unwrap();
        let use_case = UpdateStudentProfile::new(accounts.clone());

        let updated = use_case.execute(command(STUDENT, "じろう")).await.unwrap();
        assert_eq!(updated.id(), STUDENT);
        assert_eq!(updated.display_name().as_str(), "じろう");
        assert_eq!(updated.email(), before.email(), "メールアドレスはそのまま");
        assert_eq!(
            updated.as_student(),
            before.as_student(),
            "スポーツはそのまま"
        );
        assert_eq!(display_name_of(&accounts, STUDENT), "じろう");

        // 上限ちょうどまで書ける
        let full = "あ".repeat(DisplayName::MAX_CHARS);
        use_case
            .execute(command(STUDENT, full.clone()))
            .await
            .unwrap();
        assert_eq!(display_name_of(&accounts, STUDENT), full);
    }

    #[tokio::test]
    async fn empty_or_too_long_display_name_is_invalid_and_nothing_changes() {
        let accounts = accounts().await;
        let use_case = UpdateStudentProfile::new(accounts.clone());
        use_case
            .execute(command(STUDENT, "前の名前"))
            .await
            .unwrap();

        for (display_name, expected) in [
            (String::new(), DomainError::EmptyDisplayName),
            (" \u{3000}".to_string(), DomainError::EmptyDisplayName),
            (
                "あ".repeat(DisplayName::MAX_CHARS + 1),
                DomainError::DisplayNameTooLong { max: 50 },
            ),
        ] {
            let result = use_case.execute(command(STUDENT, display_name)).await;
            assert!(
                matches!(&result, Err(UpdateStudentProfileError::Invalid(error)) if *error == expected),
                "{result:?}"
            );
        }
        assert_eq!(display_name_of(&accounts, STUDENT), "前の名前");
    }

    #[tokio::test]
    async fn non_student_or_unknown_account_is_student_not_found() {
        let accounts = accounts().await;
        let before = accounts.all();
        let use_case = UpdateStudentProfile::new(accounts.clone());

        for id in [HEADQUARTERS, COACH, AccountId::from_u128(99)] {
            let result = use_case.execute(command(id, "じろう")).await;
            assert!(
                matches!(result, Err(UpdateStudentProfileError::StudentNotFound)),
                "{id:?}: {result:?}"
            );
        }
        assert_eq!(accounts.all(), before, "何も変えない");
    }
}
