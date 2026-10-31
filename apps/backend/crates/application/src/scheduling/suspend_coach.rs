use std::collections::BTreeSet;

use domain::{identity::AccountId, scheduling::CoachAcceptance};

use super::{ChangeCoachAcceptanceError, CoachAcceptanceRepository, CoachIntroductionLookup};
use crate::{Unavailable, clock::Clock};

/// コーチの受付の停止のユースケース（本部だけが呼ぶ。admin-web の `suspendCoach`）。誰が呼べるか（本部だけ）は
/// 境界（interface）が確かめる。
///
/// コーチがいるかを profile に問い合わせ（[`CoachIntroductionLookup`]。コーチでないアカウントとないアカウントは
/// [`ChangeCoachAcceptanceError::CoachNotFound`]）、domain の [`CoachAcceptance::suspend`] で止めて、止めた記録（操作した
/// 本部のアカウントと時刻）とともに保存する。ここでは判定しない。入っている予約と枠は変えない（停止中のコーチの枠は生徒の検索に出ず、新しい予約を受けない）。
/// 行動イベントも境界間イベントも発火しない（他のコンテキストは受付の状態をポートで読む）。
pub struct SuspendCoach<R, I, C> {
    acceptances: R,
    coaches: I,
    clock: C,
}

impl<R, I, C> SuspendCoach<R, I, C>
where
    R: CoachAcceptanceRepository,
    I: CoachIntroductionLookup,
    C: Clock,
{
    pub fn new(acceptances: R, coaches: I, clock: C) -> Self {
        Self {
            acceptances,
            coaches,
            clock,
        }
    }

    /// `changed_by` は操作した本部のアカウント（トークンのアカウント）。変えた時刻は時計の今。
    pub async fn execute(
        &self,
        coach_id: AccountId,
        changed_by: AccountId,
    ) -> Result<CoachAcceptance, ChangeCoachAcceptanceError> {
        ensure_coach_exists(&self.coaches, coach_id).await?;
        let changed_at = self.clock.now();
        Ok(self
            .acceptances
            .update(coach_id, |acceptance| {
                acceptance.suspend(changed_by, changed_at)
            })
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
    if coach_exists(coaches, coach_id).await? {
        Ok(())
    } else {
        Err(ChangeCoachAcceptanceError::CoachNotFound)
    }
}

/// `coach_id` のコーチがいるか（コーチの紹介が引けるか。コーチでないアカウントとないアカウントは `false`）。
/// 記録の照会（[`super::ListCoachAcceptanceChanges`]）も使う。
pub(super) async fn coach_exists<I>(coaches: &I, coach_id: AccountId) -> Result<bool, Unavailable>
where
    I: CoachIntroductionLookup,
{
    let found = coaches
        .find_coach_introductions(&BTreeSet::from([coach_id]))
        .await?;
    Ok(found.contains_key(&coach_id))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::Sport,
        identity::{Account, Email, Role},
        profile::DisplayName,
        scheduling::{CoachAcceptanceChange, CoachAcceptanceStatus, SchedulingError},
    };

    use super::*;
    use crate::{
        identity::AccountRepository,
        in_memory::{FixedClock, InMemoryAccountRepository, InMemoryCoachAcceptanceRepository},
        profile::ProfileCoachIntroductionLookup,
    };

    const COACH: AccountId = AccountId::from_u128(7);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const HEADQUARTERS: AccountId = AccountId::from_u128(2);

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(3_600)
    }

    type UseCase = SuspendCoach<
        InMemoryCoachAcceptanceRepository,
        ProfileCoachIntroductionLookup<InMemoryAccountRepository>,
        FixedClock,
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
            FixedClock(now()),
        )
    }

    #[tokio::test]
    async fn suspends_an_accepting_coach_and_records_who_and_when() {
        let use_case = use_case().await;
        let acceptance = use_case.execute(COACH, HEADQUARTERS).await.unwrap();
        assert_eq!(acceptance.coach_id(), COACH);
        assert_eq!(acceptance.status(), CoachAcceptanceStatus::Suspended);
        assert_eq!(
            use_case.acceptances().suspended_coaches().await.unwrap(),
            BTreeSet::from([COACH])
        );
        assert_eq!(
            use_case.acceptances().changes(COACH).await.unwrap(),
            vec![CoachAcceptanceChange::restore(
                COACH,
                CoachAcceptanceStatus::Suspended,
                HEADQUARTERS,
                now()
            )]
        );
    }

    #[tokio::test]
    async fn suspending_a_suspended_coach_is_rejected_without_a_second_record() {
        let use_case = use_case().await;
        use_case.execute(COACH, HEADQUARTERS).await.unwrap();
        assert!(matches!(
            use_case.execute(COACH, HEADQUARTERS).await,
            Err(ChangeCoachAcceptanceError::Invalid(
                SchedulingError::CoachAlreadySuspended
            ))
        ));
        assert_eq!(
            use_case.acceptances().suspended_coaches().await.unwrap(),
            BTreeSet::from([COACH])
        );
        assert_eq!(
            use_case.acceptances().changes(COACH).await.unwrap().len(),
            1
        );
    }

    #[tokio::test]
    async fn a_non_coach_or_missing_account_is_not_found_and_nothing_changes() {
        let use_case = use_case().await;
        for id in [STUDENT, AccountId::from_u128(99)] {
            assert!(matches!(
                use_case.execute(id, HEADQUARTERS).await,
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
