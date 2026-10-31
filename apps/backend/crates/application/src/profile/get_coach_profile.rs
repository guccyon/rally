use domain::identity::AccountId;

use crate::{
    Unavailable,
    identity::AccountRepository,
    profile::{CoachFavoriteLookup, CoachProfile},
};

/// コーチの紹介の照会の入力。`student_id` は認証したアカウント（役割の確認は境界が行う）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetCoachProfileCommand {
    pub student_id: AccountId,
    pub coach_id: AccountId,
}

/// コーチの紹介の照会のユースケース（student-app の `getCoachProfile`）。アカウントの保存先（データは当面 identity の表）
/// から読み、見た生徒のお気に入りに入っているかを matching に問い合わせて（[`CoachFavoriteLookup`]）添える。何も変えない。
///
/// `coach_id` がコーチのアカウントでなければ（生徒・本部のアカウントや、ないアカウント）`None`。誰が呼べるか（生徒だけ）と、
/// どのスポーツを見せるかは境界（interface）が決める。
pub struct GetCoachProfile<R, F> {
    accounts: R,
    favorites: F,
}

impl<R, F> GetCoachProfile<R, F>
where
    R: AccountRepository,
    F: CoachFavoriteLookup,
{
    pub fn new(accounts: R, favorites: F) -> Self {
        Self {
            accounts,
            favorites,
        }
    }

    pub async fn execute(
        &self,
        command: GetCoachProfileCommand,
    ) -> Result<Option<CoachProfile>, Unavailable> {
        let Some(account) = self.accounts.find(command.coach_id).await? else {
            return Ok(None);
        };
        let Some(coach) = account.as_coach() else {
            return Ok(None);
        };
        let is_favorite = self
            .favorites
            .is_favorite_coach(command.student_id, command.coach_id)
            .await?;
        Ok(Some(CoachProfile {
            display_name: account.display_name().clone(),
            bio: coach.bio().clone(),
            sports: coach.sports().clone(),
            is_favorite,
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, time::SystemTime};

    use domain::{
        catalog::Sport,
        identity::{Account, Email, Role},
        profile::{CoachBio, DisplayName},
    };

    use domain::matching::FavoriteCoach;

    use super::*;
    use crate::{
        in_memory::{InMemoryAccountRepository, InMemoryFavoriteCoachRepository},
        matching::MatchingCoachFavoriteLookup,
    };

    const COACH: AccountId = AccountId::from_u128(1);
    const QUIET_COACH: AccountId = AccountId::from_u128(2);
    const STUDENT: AccountId = AccountId::from_u128(3);
    const HEADQUARTERS: AccountId = AccountId::from_u128(4);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(5);

    fn account(id: AccountId, display_name: &str, role: Role, sports: &[Sport]) -> Account {
        Account::new(
            id,
            Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
            DisplayName::new(display_name).unwrap(),
            role,
            sports.iter().copied(),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    async fn accounts() -> InMemoryAccountRepository {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(
                COACH,
                "たなか",
                Role::Coach,
                &[Sport::Soccer, Sport::Tennis],
            ),
            account(QUIET_COACH, "いとう", Role::Coach, &[Sport::Golf]),
            account(STUDENT, "あおき", Role::Student, &[Sport::Tennis]),
            account(HEADQUARTERS, "うえの", Role::Headquarters, &[]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        accounts
            .update_coach_bio(COACH, CoachBio::new("テニス歴 20 年").unwrap())
            .await
            .unwrap();
        accounts
    }

    /// 生徒（3）のお気に入りは自己紹介文を書いたコーチ（1）だけ。
    async fn use_case() -> GetCoachProfile<
        InMemoryAccountRepository,
        MatchingCoachFavoriteLookup<InMemoryFavoriteCoachRepository>,
    > {
        GetCoachProfile::new(
            accounts().await,
            MatchingCoachFavoriteLookup::new(InMemoryFavoriteCoachRepository::with_favorites([
                FavoriteCoach::restore(STUDENT, COACH, SystemTime::UNIX_EPOCH),
            ])),
        )
    }

    fn command(student_id: AccountId, coach_id: AccountId) -> GetCoachProfileCommand {
        GetCoachProfileCommand {
            student_id,
            coach_id,
        }
    }

    #[tokio::test]
    async fn returns_the_display_name_bio_and_sports_of_a_coach() {
        let use_case = use_case().await;

        assert_eq!(
            use_case.execute(command(STUDENT, COACH)).await.unwrap(),
            Some(CoachProfile {
                display_name: DisplayName::new("たなか").unwrap(),
                bio: CoachBio::new("テニス歴 20 年").unwrap(),
                sports: BTreeSet::from([Sport::Tennis, Sport::Soccer]),
                is_favorite: true,
            })
        );
        assert_eq!(
            use_case
                .execute(command(STUDENT, QUIET_COACH))
                .await
                .unwrap(),
            Some(CoachProfile {
                display_name: DisplayName::new("いとう").unwrap(),
                bio: CoachBio::default(),
                sports: BTreeSet::from([Sport::Golf]),
                is_favorite: false,
            }),
            "自己紹介文を書いていなければ空。ゴルフも含めて返す（見せるかは境界が決める）"
        );
    }

    #[tokio::test]
    async fn is_favorite_is_per_student() {
        let profile = use_case()
            .await
            .execute(command(OTHER_STUDENT, COACH))
            .await
            .unwrap()
            .unwrap();
        assert!(!profile.is_favorite, "ほかの生徒のお気に入りは見ない");
    }

    #[tokio::test]
    async fn non_coach_or_unknown_account_is_none() {
        let use_case = use_case().await;

        for id in [STUDENT, HEADQUARTERS, AccountId::from_u128(99)] {
            assert_eq!(
                use_case.execute(command(STUDENT, id)).await.unwrap(),
                None,
                "{id:?}"
            );
        }
    }
}
