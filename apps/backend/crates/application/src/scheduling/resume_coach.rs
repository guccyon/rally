use domain::{identity::AccountId, scheduling::CoachAcceptance};

use super::{
    ChangeCoachAcceptanceError, CoachAcceptanceRepository, CoachIntroductionLookup,
    suspend_coach::ensure_coach_exists,
};

/// コーチの受付の再開のユースケース（本部だけが呼ぶ。admin-web の `resumeCoach`）。誰が呼べるか（本部だけ）は
/// 境界（interface）が確かめる。
///
/// コーチがいるかを profile に問い合わせ（[`CoachIntroductionLookup`]。コーチでないアカウントとないアカウントは
/// [`ChangeCoachAcceptanceError::CoachNotFound`]）、domain の [`CoachAcceptance::resume`] で受付中に戻して保存する。
/// ここでは判定しない。再開したコーチの公開中の枠は、また生徒の検索に出て予約できる。行動イベントも境界間イベントも
/// 発火しない。
pub struct ResumeCoach<R, I> {
    acceptances: R,
    coaches: I,
}

impl<R, I> ResumeCoach<R, I>
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
            .update(coach_id, CoachAcceptance::resume)
            .await?)
    }

    pub fn acceptances(&self) -> &R {
        &self.acceptances
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use domain::{
        profile::{CoachBio, DisplayName},
        scheduling::{CoachAcceptanceStatus, SchedulingError},
    };

    use super::*;
    use crate::{
        Unavailable, in_memory::InMemoryCoachAcceptanceRepository, scheduling::CoachIntroduction,
    };

    const COACH: AccountId = AccountId::from_u128(7);

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

    fn use_case() -> ResumeCoach<InMemoryCoachAcceptanceRepository, OneCoach> {
        ResumeCoach::new(InMemoryCoachAcceptanceRepository::default(), OneCoach)
    }

    #[tokio::test]
    async fn resumes_a_suspended_coach() {
        let use_case = use_case();
        use_case
            .acceptances()
            .update(COACH, CoachAcceptance::suspend)
            .await
            .unwrap();

        let acceptance = use_case.execute(COACH).await.unwrap();
        assert_eq!(acceptance.status(), CoachAcceptanceStatus::Accepting);
        assert!(
            use_case
                .acceptances()
                .suspended_coaches()
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn resuming_an_accepting_coach_is_rejected() {
        assert!(matches!(
            use_case().execute(COACH).await,
            Err(ChangeCoachAcceptanceError::Invalid(
                SchedulingError::CoachNotSuspended
            ))
        ));
    }

    #[tokio::test]
    async fn a_missing_coach_is_not_found() {
        assert!(matches!(
            use_case().execute(AccountId::from_u128(99)).await,
            Err(ChangeCoachAcceptanceError::CoachNotFound)
        ));
    }
}
