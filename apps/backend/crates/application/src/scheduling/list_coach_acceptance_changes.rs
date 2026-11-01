use domain::{identity::AccountId, scheduling::CoachAcceptanceChange};

use super::{CoachAcceptanceRepository, CoachIntroductionLookup, suspend_coach::coach_exists};
use crate::Unavailable;

/// コーチ 1 人の受付を止めた・再開した記録の照会のユースケース（本部だけが呼ぶ。admin-web の
/// `listCoachAcceptanceChanges`）。誰が呼べるか（本部だけ）は境界（interface）が確かめる。照会だけで、何も保存しない。
///
/// コーチがいるかを profile に問い合わせ（[`CoachIntroductionLookup`]。停止・再開と同じ）、いれば保存先が返す記録
/// （新しい順。[`CoachAcceptanceRepository::changes`]）をそのまま返す。コーチでないアカウントとないアカウントは `None`。
/// 一度も止めていないコーチは空の一覧。
pub struct ListCoachAcceptanceChanges<R, I> {
    acceptances: R,
    coaches: I,
}

impl<R, I> ListCoachAcceptanceChanges<R, I>
where
    R: CoachAcceptanceRepository,
    I: CoachIntroductionLookup,
{
    pub fn new(acceptances: R, coaches: I) -> Self {
        Self {
            acceptances,
            coaches,
        }
    }

    pub async fn execute(
        &self,
        coach_id: AccountId,
    ) -> Result<Option<Vec<CoachAcceptanceChange>>, Unavailable> {
        if !coach_exists(&self.coaches, coach_id).await? {
            return Ok(None);
        }
        Ok(Some(self.acceptances.changes(coach_id).await?))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::Sport,
        identity::{Account, Email, Role},
        profile::DisplayName,
        scheduling::{CoachAcceptanceStatus, CoachSuspensionReason},
    };

    use super::*;
    use crate::{
        identity::AccountRepository,
        in_memory::{InMemoryAccountRepository, InMemoryCoachAcceptanceRepository},
        profile::ProfileCoachIntroductionLookup,
    };

    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const HEADQUARTERS: AccountId = AccountId::from_u128(2);
    const OTHER_HEADQUARTERS: AccountId = AccountId::from_u128(3);

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn reason(text: &str) -> CoachSuspensionReason {
        CoachSuspensionReason::new(text).unwrap()
    }

    type UseCase = ListCoachAcceptanceChanges<
        InMemoryCoachAcceptanceRepository,
        ProfileCoachIntroductionLookup<InMemoryAccountRepository>,
    >;

    async fn use_case(acceptances: InMemoryCoachAcceptanceRepository) -> UseCase {
        let accounts = InMemoryAccountRepository::default();
        for (id, role) in [
            (COACH, Role::Coach),
            (OTHER_COACH, Role::Coach),
            (STUDENT, Role::Student),
        ] {
            accounts
                .insert(
                    Account::new(
                        id,
                        Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
                        DisplayName::new("たなか").unwrap(),
                        role,
                        [Sport::Tennis],
                        SystemTime::UNIX_EPOCH,
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
        }
        ListCoachAcceptanceChanges::new(acceptances, ProfileCoachIntroductionLookup::new(accounts))
    }

    #[tokio::test]
    async fn lists_the_changes_of_the_coach_newest_first_with_the_reasons() {
        let acceptances = InMemoryCoachAcceptanceRepository::default();
        acceptances
            .update(COACH, |a| {
                a.suspend(HEADQUARTERS, reason("無断欠席が続いた"), at(10))
            })
            .await
            .unwrap();
        acceptances
            .update(OTHER_COACH, |a| {
                a.suspend(HEADQUARTERS, CoachSuspensionReason::default(), at(15))
            })
            .await
            .unwrap();
        acceptances
            .update(COACH, |a| a.resume(OTHER_HEADQUARTERS, at(20)))
            .await
            .unwrap();
        // 同じ時刻なら後に保存した方が先
        acceptances
            .update(COACH, |a| {
                a.suspend(HEADQUARTERS, CoachSuspensionReason::default(), at(20))
            })
            .await
            .unwrap();

        let changes = use_case(acceptances)
            .await
            .execute(COACH)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            changes,
            vec![
                CoachAcceptanceChange::restore(
                    COACH,
                    CoachAcceptanceStatus::Suspended,
                    Some(CoachSuspensionReason::default()),
                    HEADQUARTERS,
                    at(20)
                ),
                CoachAcceptanceChange::restore(
                    COACH,
                    CoachAcceptanceStatus::Accepting,
                    None,
                    OTHER_HEADQUARTERS,
                    at(20)
                ),
                CoachAcceptanceChange::restore(
                    COACH,
                    CoachAcceptanceStatus::Suspended,
                    Some(reason("無断欠席が続いた")),
                    HEADQUARTERS,
                    at(10)
                ),
            ]
        );
    }

    #[tokio::test]
    async fn a_coach_never_suspended_has_no_changes() {
        let changes = use_case(InMemoryCoachAcceptanceRepository::default())
            .await
            .execute(COACH)
            .await
            .unwrap();
        assert_eq!(changes, Some(Vec::new()));
    }

    #[tokio::test]
    async fn a_non_coach_or_missing_account_is_none() {
        let use_case = use_case(InMemoryCoachAcceptanceRepository::default()).await;
        for id in [STUDENT, AccountId::from_u128(99)] {
            assert_eq!(use_case.execute(id).await.unwrap(), None);
        }
    }
}
