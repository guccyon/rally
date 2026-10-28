use domain::{
    DomainError,
    identity::{Account, AccountId, CoachBio},
};

use super::AccountRepository;
use crate::Unavailable;

/// コーチの自己紹介の変更の入力（検証前の値）。`coach_id` は認証したアカウント（役割の確認は境界が行う）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateCoachProfileCommand {
    pub coach_id: AccountId,
    pub bio: String,
}

#[derive(Debug)]
pub enum UpdateCoachProfileError {
    /// 入力がドメインの不変条件を満たさない（自己紹介文が長すぎる）
    Invalid(DomainError),
    /// `coach_id` のコーチのアカウントがない
    CoachNotFound,
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<DomainError> for UpdateCoachProfileError {
    fn from(error: DomainError) -> Self {
        Self::Invalid(error)
    }
}

impl From<Unavailable> for UpdateCoachProfileError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}

/// 自分の自己紹介の変更のユースケース（admin-web の `updateMyCoachProfile`）。自己紹介文を置き換え、変えたあとの
/// アカウントを返す。空にもできる。行動イベントも境界間イベントも発火しない。
pub struct UpdateCoachProfile<R> {
    accounts: R,
}

impl<R> UpdateCoachProfile<R>
where
    R: AccountRepository,
{
    pub fn new(accounts: R) -> Self {
        Self { accounts }
    }

    pub async fn execute(
        &self,
        command: UpdateCoachProfileCommand,
    ) -> Result<Account, UpdateCoachProfileError> {
        let bio = CoachBio::new(command.bio)?;
        self.accounts
            .update_coach_bio(command.coach_id, bio)
            .await?
            .ok_or(UpdateCoachProfileError::CoachNotFound)
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::identity::{DisplayName, Email, Role, Sport};

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
            account(COACH, Role::Coach, &[Sport::Tennis, Sport::Golf]),
            account(STUDENT, Role::Student, &[Sport::Soccer]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        accounts
    }

    fn command(coach_id: AccountId, bio: impl Into<String>) -> UpdateCoachProfileCommand {
        UpdateCoachProfileCommand {
            coach_id,
            bio: bio.into(),
        }
    }

    fn bio_of(accounts: &InMemoryAccountRepository, id: AccountId) -> Option<String> {
        accounts
            .all()
            .into_iter()
            .find(|a| a.id() == id)
            .and_then(|a| a.as_coach().map(|c| c.bio().as_str().to_string()))
    }

    #[tokio::test]
    async fn replaces_the_bio_and_returns_the_updated_account() {
        let accounts = accounts().await;
        let use_case = UpdateCoachProfile::new(accounts.clone());

        let updated = use_case
            .execute(command(COACH, "テニス歴 20 年。初心者歓迎"))
            .await
            .unwrap();
        assert_eq!(updated.id(), COACH);
        let coach = updated.as_coach().unwrap();
        assert_eq!(coach.bio().as_str(), "テニス歴 20 年。初心者歓迎");
        assert_eq!(coach.sports().len(), 2, "提供スポーツはそのまま");
        assert_eq!(
            bio_of(&accounts, COACH).as_deref(),
            Some("テニス歴 20 年。初心者歓迎")
        );

        // 上限ちょうどまで書け、空に戻せる
        let full = "あ".repeat(CoachBio::MAX_CHARS);
        use_case
            .execute(command(COACH, full.clone()))
            .await
            .unwrap();
        assert_eq!(bio_of(&accounts, COACH), Some(full));
        let cleared = use_case.execute(command(COACH, "")).await.unwrap();
        assert_eq!(cleared.as_coach().unwrap().bio(), &CoachBio::default());
        assert_eq!(bio_of(&accounts, COACH).as_deref(), Some(""));
    }

    #[tokio::test]
    async fn too_long_bio_is_invalid_and_nothing_changes() {
        let accounts = accounts().await;
        let use_case = UpdateCoachProfile::new(accounts.clone());
        use_case.execute(command(COACH, "前の文")).await.unwrap();

        let result = use_case
            .execute(command(COACH, "あ".repeat(CoachBio::MAX_CHARS + 1)))
            .await;
        assert!(
            matches!(
                result,
                Err(UpdateCoachProfileError::Invalid(
                    DomainError::CoachBioTooLong { max: 400 }
                ))
            ),
            "{result:?}"
        );
        assert_eq!(bio_of(&accounts, COACH).as_deref(), Some("前の文"));
    }

    #[tokio::test]
    async fn non_coach_or_unknown_account_is_coach_not_found() {
        let accounts = accounts().await;
        let before = accounts.all();
        let use_case = UpdateCoachProfile::new(accounts.clone());

        for id in [HEADQUARTERS, STUDENT, AccountId::from_u128(99)] {
            let result = use_case.execute(command(id, "自己紹介")).await;
            assert!(
                matches!(result, Err(UpdateCoachProfileError::CoachNotFound)),
                "{id:?}: {result:?}"
            );
        }
        assert_eq!(accounts.all(), before, "何も変えない");
    }
}
