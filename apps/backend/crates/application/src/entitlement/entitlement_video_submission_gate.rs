use domain::identity::AccountId;

use super::EntitlementRepository;
use crate::{Unavailable, async_lesson::VideoSubmissionGate, lesson::LessonCompletionRepository};

/// async-lesson の [`VideoSubmissionGate`] の entitlement による実装。
///
/// 月額契約（送り放題。domain の [`domain::entitlement::Entitlement::unlimited_video_and_chat`]）の生徒は常に通す。
/// それ以外（契約の前）は体験として、送った動画も完了したレッスン（同期・非同期のどちらも）もないときだけ通す。
/// 受け終えた体験予約をあとでキャンセルしても完了記録は残るので通さない。読むだけで何も変えない。
#[derive(Debug, Clone)]
pub struct EntitlementVideoSubmissionGate<E, R> {
    entitlements: E,
    completions: R,
}

impl<E, R> EntitlementVideoSubmissionGate<E, R> {
    pub fn new(entitlements: E, completions: R) -> Self {
        Self {
            entitlements,
            completions,
        }
    }
}

impl<E, R> VideoSubmissionGate for EntitlementVideoSubmissionGate<E, R>
where
    E: EntitlementRepository,
    R: LessonCompletionRepository,
{
    async fn may_submit_video(
        &self,
        student_id: AccountId,
        sent_videos: usize,
    ) -> Result<bool, Unavailable> {
        if self
            .entitlements
            .find_by_student(student_id)
            .await?
            .is_some_and(|entitlement| entitlement.unlimited_video_and_chat())
        {
            return Ok(true);
        }
        if sent_videos > 0 {
            return Ok(false);
        }
        Ok(self
            .completions
            .completions_of_student(student_id)
            .await?
            .is_empty())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::VideoId,
        catalog::Sport,
        entitlement::{
            Entitlement, EntitlementId, LiveLessonQuota, SubscriptionId, SubscriptionPlan,
            YearMonth,
        },
        lesson::{LessonCompletion, LessonCompletionId, LessonSource},
        live_lesson::SessionId,
    };

    use super::*;
    use crate::in_memory::{InMemoryEntitlementRepository, InMemoryLessonCompletionRepository};

    const MONTHLY: AccountId = AccountId::from_u128(1);
    const TRIAL_DONE: AccountId = AccountId::from_u128(2);
    const FRESH: AccountId = AccountId::from_u128(3);
    const COACH: AccountId = AccountId::from_u128(7);

    fn completion(id: u128, student: AccountId, source: LessonSource) -> LessonCompletion {
        LessonCompletion::restore(
            LessonCompletionId::from_u128(id),
            student,
            COACH,
            source,
            SystemTime::UNIX_EPOCH + Duration::from_secs(3600),
            true,
        )
    }

    fn gate() -> EntitlementVideoSubmissionGate<
        InMemoryEntitlementRepository,
        InMemoryLessonCompletionRepository,
    > {
        let october = YearMonth::new(2026, 10).unwrap();
        EntitlementVideoSubmissionGate::new(
            InMemoryEntitlementRepository::with_entitlements([Entitlement::monthly(
                EntitlementId::from_u128(10),
                MONTHLY,
                Sport::Tennis,
                SubscriptionId::from_u128(20),
                LiveLessonQuota::fresh(SubscriptionPlan::Standard, october),
            )
            .unwrap()]),
            InMemoryLessonCompletionRepository::with_completions([
                completion(1, TRIAL_DONE, LessonSource::Live(SessionId::from_u128(1))),
                completion(2, MONTHLY, LessonSource::Async(VideoId::from_u128(2))),
            ]),
        )
    }

    #[tokio::test]
    async fn monthly_students_may_always_submit() {
        let gate = gate();
        assert!(gate.may_submit_video(MONTHLY, 0).await.unwrap());
        assert!(gate.may_submit_video(MONTHLY, 5).await.unwrap());
    }

    #[tokio::test]
    async fn a_student_without_a_contract_may_submit_one_trial_video() {
        let gate = gate();
        assert!(gate.may_submit_video(FRESH, 0).await.unwrap());
        assert!(!gate.may_submit_video(FRESH, 1).await.unwrap());
    }

    #[tokio::test]
    async fn a_student_who_completed_a_lesson_may_not_submit_even_without_videos() {
        assert!(!gate().may_submit_video(TRIAL_DONE, 0).await.unwrap());
    }
}
