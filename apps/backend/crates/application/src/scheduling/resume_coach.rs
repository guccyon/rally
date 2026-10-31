use domain::{identity::AccountId, scheduling::CoachAcceptance};

use super::{
    ChangeCoachAcceptanceError, CoachAcceptanceRepository, CoachIntroductionLookup,
    suspend_coach::ensure_coach_exists,
};
use crate::clock::Clock;

/// コーチの受付の再開のユースケース（本部だけが呼ぶ。admin-web の `resumeCoach`）。誰が呼べるか（本部だけ）は
/// 境界（interface）が確かめる。
///
/// コーチがいるかを profile に問い合わせ（[`CoachIntroductionLookup`]。コーチでないアカウントとないアカウントは
/// [`ChangeCoachAcceptanceError::CoachNotFound`]）、domain の [`CoachAcceptance::resume`] で受付中に戻して、再開した記録
/// （操作した本部のアカウントと時刻）とともに保存する。ここでは判定しない。再開したコーチの公開中の枠は、また生徒の検索に出て予約できる。行動イベントも境界間イベントも
/// 発火しない。
pub struct ResumeCoach<R, I, C> {
    acceptances: R,
    coaches: I,
    clock: C,
}

impl<R, I, C> ResumeCoach<R, I, C>
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
                acceptance.resume(changed_by, changed_at)
            })
            .await?)
    }

    pub fn acceptances(&self) -> &R {
        &self.acceptances
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        time::{Duration, SystemTime},
    };

    use domain::{
        profile::{CoachBio, DisplayName},
        scheduling::{CoachAcceptanceChange, CoachAcceptanceStatus, SchedulingError},
    };

    use super::*;
    use crate::{
        Unavailable,
        in_memory::{FixedClock, InMemoryCoachAcceptanceRepository},
        scheduling::CoachIntroduction,
    };

    const COACH: AccountId = AccountId::from_u128(7);
    const HEADQUARTERS: AccountId = AccountId::from_u128(1);

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(3_600)
    }

    /// `COACH` だけがいる profile。
    struct OneCoach;

    impl CoachIntroductionLookup for OneCoach {
        async fn find_coach_introductions(
            &self,
            coach_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, CoachIntroduction>, Unavailable> {
            Ok(coach_ids
                .iter()
                .filter(|id| **id == COACH)
                .map(|id| {
                    (
                        *id,
                        CoachIntroduction {
                            display_name: DisplayName::new("たなか").unwrap(),
                            bio: CoachBio::default(),
                        },
                    )
                })
                .collect())
        }
    }

    fn use_case() -> ResumeCoach<InMemoryCoachAcceptanceRepository, OneCoach, FixedClock> {
        ResumeCoach::new(
            InMemoryCoachAcceptanceRepository::default(),
            OneCoach,
            FixedClock(now()),
        )
    }

    #[tokio::test]
    async fn resumes_a_suspended_coach_and_records_who_and_when() {
        let use_case = use_case();
        let suspended_at = SystemTime::UNIX_EPOCH;
        use_case
            .acceptances()
            .update(COACH, |a| a.suspend(HEADQUARTERS, suspended_at))
            .await
            .unwrap();

        let acceptance = use_case.execute(COACH, HEADQUARTERS).await.unwrap();
        assert_eq!(acceptance.status(), CoachAcceptanceStatus::Accepting);
        assert!(
            use_case
                .acceptances()
                .suspended_coaches()
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            use_case.acceptances().changes(COACH).await.unwrap(),
            vec![
                CoachAcceptanceChange::restore(
                    COACH,
                    CoachAcceptanceStatus::Accepting,
                    HEADQUARTERS,
                    now()
                ),
                CoachAcceptanceChange::restore(
                    COACH,
                    CoachAcceptanceStatus::Suspended,
                    HEADQUARTERS,
                    suspended_at
                ),
            ],
            "新しい順"
        );
    }

    #[tokio::test]
    async fn resuming_an_accepting_coach_is_rejected_without_a_record() {
        let use_case = use_case();
        assert!(matches!(
            use_case.execute(COACH, HEADQUARTERS).await,
            Err(ChangeCoachAcceptanceError::Invalid(
                SchedulingError::CoachNotSuspended
            ))
        ));
        assert!(
            use_case
                .acceptances()
                .changes(COACH)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_missing_coach_is_not_found() {
        assert!(matches!(
            use_case()
                .execute(AccountId::from_u128(99), HEADQUARTERS)
                .await,
            Err(ChangeCoachAcceptanceError::CoachNotFound)
        ));
    }
}
