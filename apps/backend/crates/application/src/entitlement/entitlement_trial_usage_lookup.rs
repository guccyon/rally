use domain::identity::AccountId;

use super::EntitlementRepository;
use crate::{
    Unavailable, async_lesson::VideoRepository, lesson::LessonCompletionRepository,
    scheduling::TrialUsageLookup,
};

/// scheduling の [`TrialUsageLookup`] の entitlement による実装（体験は通話か動画添削のどちらか 1 回。ADR 0014）。
///
/// 月額契約（domain の [`domain::entitlement::Entitlement::unlimited_video_and_chat`]）の生徒は体験の対象ではないので
/// 使用済みとする。契約の前の生徒は、完了したレッスン（同期・非同期のどちらも）か送った動画（添削待ちも返却済みも）が
/// 1 件でもあれば使用済み。動画の送付の判定（[`super::EntitlementVideoSubmissionGate`]）と同じ事実を見る。
/// 読むだけで何も変えない。
#[derive(Debug, Clone)]
pub struct EntitlementTrialUsageLookup<E, R, V> {
    entitlements: E,
    completions: R,
    videos: V,
}

impl<E, R, V> EntitlementTrialUsageLookup<E, R, V> {
    pub fn new(entitlements: E, completions: R, videos: V) -> Self {
        Self {
            entitlements,
            completions,
            videos,
        }
    }
}

impl<E, R, V> TrialUsageLookup for EntitlementTrialUsageLookup<E, R, V>
where
    E: EntitlementRepository,
    R: LessonCompletionRepository,
    V: VideoRepository,
{
    async fn has_used_trial_outside_bookings(
        &self,
        student_id: AccountId,
    ) -> Result<bool, Unavailable> {
        if self
            .entitlements
            .find_by_student(student_id)
            .await?
            .is_some_and(|entitlement| entitlement.unlimited_video_and_chat())
        {
            return Ok(true);
        }
        if !self
            .completions
            .completions_of_student(student_id)
            .await?
            .is_empty()
        {
            return Ok(true);
        }
        Ok(!self.videos.videos_of_student(student_id).await?.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::{Video, VideoId},
        catalog::Sport,
        entitlement::{
            Entitlement, EntitlementId, LiveLessonQuota, SubscriptionId, SubscriptionPlan,
            YearMonth,
        },
        lesson::{LessonCompletion, LessonCompletionId, LessonSource},
        live_lesson::SessionId,
    };

    use super::*;
    use crate::in_memory::{
        InMemoryEntitlementRepository, InMemoryLessonCompletionRepository, InMemoryVideoRepository,
    };

    const MONTHLY: AccountId = AccountId::from_u128(1);
    const LESSON_DONE: AccountId = AccountId::from_u128(2);
    const VIDEO_SENT: AccountId = AccountId::from_u128(3);
    const FRESH: AccountId = AccountId::from_u128(4);
    const COACH: AccountId = AccountId::from_u128(7);

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(hour * 3600)
    }

    fn lookup() -> EntitlementTrialUsageLookup<
        InMemoryEntitlementRepository,
        InMemoryLessonCompletionRepository,
        InMemoryVideoRepository,
    > {
        EntitlementTrialUsageLookup::new(
            InMemoryEntitlementRepository::with_entitlements([Entitlement::monthly(
                EntitlementId::from_u128(10),
                MONTHLY,
                Sport::Basketball,
                SubscriptionId::from_u128(20),
                LiveLessonQuota::fresh(
                    SubscriptionPlan::Standard,
                    YearMonth::new(2026, 11).unwrap(),
                ),
            )
            .unwrap()]),
            InMemoryLessonCompletionRepository::with_completions([LessonCompletion::restore(
                LessonCompletionId::from_u128(1),
                LESSON_DONE,
                COACH,
                LessonSource::Live(SessionId::from_u128(1)),
                at(1),
                true,
            )]),
            InMemoryVideoRepository::with_videos([Video::submit(
                VideoId::from_u128(1),
                VIDEO_SENT,
                COACH,
                Sport::Tennis,
                at(1),
            )]),
        )
    }

    #[tokio::test]
    async fn a_monthly_student_has_used_the_trial_even_without_lessons_or_videos() {
        assert!(
            lookup()
                .has_used_trial_outside_bookings(MONTHLY)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn a_completed_lesson_or_a_sent_video_uses_the_trial() {
        let lookup = lookup();
        assert!(
            lookup
                .has_used_trial_outside_bookings(LESSON_DONE)
                .await
                .unwrap()
        );
        assert!(
            lookup
                .has_used_trial_outside_bookings(VIDEO_SENT)
                .await
                .unwrap(),
            "添削待ちの動画でも使用済み"
        );
    }

    #[tokio::test]
    async fn a_student_without_a_contract_lessons_or_videos_has_not_used_the_trial() {
        assert!(
            !lookup()
                .has_used_trial_outside_bookings(FRESH)
                .await
                .unwrap()
        );
    }
}
