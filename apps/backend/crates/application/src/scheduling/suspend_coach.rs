use std::collections::BTreeSet;

use domain::{identity::AccountId, scheduling::CoachAcceptance};

use super::{ChangeCoachAcceptanceError, CoachAcceptanceRepository, CoachIntroductionLookup};

/// コーチの受付の停止のユースケース（本部だけが呼ぶ。admin-web の `suspendCoach`）。誰が呼べるか（本部だけ）は
/// 境界（interface）が確かめる。
///
/// コーチがいるかを profile に問い合わせ（[`CoachIntroductionLookup`]。コーチでないアカウントとないアカウントは
/// [`ChangeCoachAcceptanceError::CoachNotFound`]）、domain の [`CoachAcceptance::suspend`] で止めて保存する。
/// ここでは判定しない。入っている予約と枠は変えない（停止中のコーチの枠は生徒の検索に出ず、新しい予約を受けない）。
/// 行動イベントも境界間イベントも発火しない（他のコンテキストは受付の状態をポートで読む）。
pub struct SuspendCoach<R, I> {
    acceptances: R,
    coaches: I,
}

impl<R, I> SuspendCoach<R, I>
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
    ) -> Result<CoachAcceptance, ChangeCoachAcceptanceError> {
        ensure_coach_exists(&self.coaches, coach_id).await?;
        Ok(self
            .acceptances
            .update(coach_id, CoachAcceptance::suspend)
            .await?)
    }

    pub fn acceptances(&self) -> &R {
        &self.acceptances
    }
}

/// `coach_id` のコーチがいるか（コーチの紹介が引けるか）。いなければ [`ChangeCoachAcceptanceError::CoachNotFound`]。
/// 停止と再開（[`super::ResumeCoach`]）が共有する。
pub(super) async fn ensure_coach_exists<I>(
    coaches: &I,
    coach_id: AccountId,
) -> Result<(), ChangeCoachAcceptanceError>
where
    I: CoachIntroductionLookup,
{
    let found = coaches
        .find_coach_introductions(&BTreeSet::from([coach_id]))
        .await?;
    if found.contains_key(&coach_id) {
        Ok(())
    } else {
        Err(ChangeCoachAcceptanceError::CoachNotFound)
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::{
        catalog::Sport,
        identity::{Account, Email, Role},
        profile::DisplayName,
        scheduling::{CoachAcceptanceStatus, SchedulingError},
    };

    use super::*;
    use crate::{
        identity::AccountRepository,
        in_memory::{InMemoryAccountRepository, InMemoryCoachAcceptanceRepository},
        profile::ProfileCoachIntroductionLookup,
    };

    const COACH: AccountId = AccountId::from_u128(7);
    const STUDENT: AccountId = AccountId::from_u128(1);

    type UseCase = SuspendCoach<
        InMemoryCoachAcceptanceRepository,
        ProfileCoachIntroductionLookup<InMemoryAccountRepository>,
    >;

    async fn use_case() -> UseCase {
        let accounts = InMemoryAccountRepository::default();
        for (id, role) in [(COACH, Role::Coach), (STUDENT, Role::Student)] {
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
        SuspendCoach::new(
            InMemoryCoachAcceptanceRepository::default(),
            ProfileCoachIntroductionLookup::new(accounts),
        )
    }

    #[tokio::test]
    async fn suspends_an_accepting_coach() {
        let use_case = use_case().await;
        let acceptance = use_case.execute(COACH).await.unwrap();
        assert_eq!(acceptance.coach_id(), COACH);
        assert_eq!(acceptance.status(), CoachAcceptanceStatus::Suspended);
        assert_eq!(
            use_case.acceptances().suspended_coaches().await.unwrap(),
            BTreeSet::from([COACH])
        );
    }

    #[tokio::test]
    async fn suspending_a_suspended_coach_is_rejected() {
        let use_case = use_case().await;
        use_case.execute(COACH).await.unwrap();
        assert!(matches!(
            use_case.execute(COACH).await,
            Err(ChangeCoachAcceptanceError::Invalid(
                SchedulingError::CoachAlreadySuspended
            ))
        ));
        assert_eq!(
            use_case.acceptances().suspended_coaches().await.unwrap(),
            BTreeSet::from([COACH])
        );
    }

    #[tokio::test]
    async fn a_non_coach_or_missing_account_is_not_found_and_nothing_changes() {
        let use_case = use_case().await;
        for id in [STUDENT, AccountId::from_u128(99)] {
            assert!(matches!(
                use_case.execute(id).await,
                Err(ChangeCoachAcceptanceError::CoachNotFound)
            ));
        }
        assert!(
            use_case
                .acceptances()
                .suspended_coaches()
                .await
                .unwrap()
                .is_empty()
        );
    }
}
