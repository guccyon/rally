use domain::{entitlement::TrialUsage, identity::AccountId};

use super::{EntitlementRepository, TrialUsageLookup};
use crate::{Unavailable, async_lesson::VideoRepository, lesson::LessonCompletionRepository};

/// [`TrialUsageLookup`] の実装。受講権の保存先から月額の受講権（動画とチャットの送り放題。domain の
/// [`domain::entitlement::Entitlement::unlimited_video_and_chat`]）の有無を、完了記録の保存先から完了したレッスン
/// （同期・非同期のどちらでも。docs/adr/0006）の有無を、動画の保存先から送った動画（添削待ちも返却済みも含む）の
/// 有無を読む。読むだけで何も変えない。
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
    async fn find_trial_usage(&self, student_id: AccountId) -> Result<TrialUsage, Unavailable> {
        let has_monthly_entitlement = self
            .entitlements
            .find_by_student(student_id)
            .await?
            .is_some_and(|entitlement| entitlement.unlimited_video_and_chat());
        let has_completed_lesson = !self
            .completions
            .completions_of_student(student_id)
            .await?
            .is_empty();
        let has_submitted_video = !self.videos.videos_of_student(student_id).await?.is_empty();
        Ok(TrialUsage::default()
            .with_monthly_entitlement(has_monthly_entitlement)
            .with_completed_lesson(has_completed_lesson)
            .with_submitted_video(has_submitted_video))
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

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

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(9);

    fn lookup(
        entitlements: impl IntoIterator<Item = Entitlement>,
        completions: impl IntoIterator<Item = LessonCompletion>,
        videos: impl IntoIterator<Item = Video>,
    ) -> EntitlementTrialUsageLookup<
        InMemoryEntitlementRepository,
        InMemoryLessonCompletionRepository,
        InMemoryVideoRepository,
    > {
        EntitlementTrialUsageLookup::new(
            InMemoryEntitlementRepository::with_entitlements(entitlements),
            InMemoryLessonCompletionRepository::with_completions(completions),
            InMemoryVideoRepository::with_videos(videos),
        )
    }

    fn monthly(student_id: AccountId) -> Entitlement {
        Entitlement::monthly(
            EntitlementId::from_u128(10),
            student_id,
            Sport::Tennis,
            SubscriptionId::from_u128(20),
            LiveLessonQuota::fresh(
                SubscriptionPlan::Standard,
                YearMonth::new(2026, 10).unwrap(),
            ),
        )
        .unwrap()
    }

    fn video(student_id: AccountId) -> Video {
        Video::submit(
            VideoId::from_u128(30),
            student_id,
            COACH,
            Sport::Tennis,
            SystemTime::UNIX_EPOCH,
        )
    }

    fn completion(student_id: AccountId) -> LessonCompletion {
        LessonCompletion::restore(
            LessonCompletionId::from_u128(40),
            student_id,
            COACH,
            LessonSource::Live(SessionId::from_u128(50)),
            SystemTime::UNIX_EPOCH,
            true,
        )
    }

    #[tokio::test]
    async fn reads_each_material_of_the_student() {
        let unused = TrialUsage::default();
        assert_eq!(
            lookup([], [], []).find_trial_usage(STUDENT).await.unwrap(),
            unused
        );
        assert_eq!(
            lookup([monthly(STUDENT)], [], [])
                .find_trial_usage(STUDENT)
                .await
                .unwrap(),
            unused.with_monthly_entitlement(true)
        );
        assert_eq!(
            lookup([], [completion(STUDENT)], [])
                .find_trial_usage(STUDENT)
                .await
                .unwrap(),
            unused.with_completed_lesson(true)
        );
        assert_eq!(
            lookup([], [], [video(STUDENT)])
                .find_trial_usage(STUDENT)
                .await
                .unwrap(),
            unused.with_submitted_video(true),
            "添削待ちの動画も送った動画"
        );
    }

    #[tokio::test]
    async fn ignores_other_students() {
        let lookup = lookup(
            [monthly(OTHER_STUDENT)],
            [completion(OTHER_STUDENT)],
            [video(OTHER_STUDENT)],
        );
        assert_eq!(
            lookup.find_trial_usage(STUDENT).await.unwrap(),
            TrialUsage::default()
        );
    }
}
