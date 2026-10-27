use domain::{
    DomainError,
    identity::AccountId,
    lesson::LessonId,
    review::{LessonReview, LessonReviewComment, Rating},
};

use super::{CompletedLessonLookup, InsertReviewError, LessonReviewRepository};
use crate::{
    Unavailable,
    analytics::{AnalyticsEvent, AnalyticsSink},
    clock::Clock,
};

/// レッスンの評価の入力。`account_id` は認証したアカウント（レッスンの生徒かは domain が確かめる）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitLessonReviewCommand {
    pub account_id: AccountId,
    pub lesson_id: LessonId,
    pub rating: Rating,
    pub comment: LessonReviewComment,
}

#[derive(Debug)]
pub enum SubmitLessonReviewError {
    /// 完了したレッスンがない
    LessonNotFound,
    /// 評価がドメインの不変条件を満たさない（レッスンの生徒でない、評価済み）
    Invalid(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// レッスンの評価のユースケース（student-app の `submitLessonReview`）。
///
/// 完了したレッスンを問い合わせ（[`CompletedLessonLookup`]）、そのレッスンの既存の評価と合わせて domain の
/// [`LessonReview::submit`] で評価を作って保存する。保存したら行動イベント `review.submitted` を 1 回発火する。
/// 拒否・失敗では発火しない。
///
/// 同じレッスンの評価が並行して両方 domain の判定を通っても、保存先が後の方を拒む（409 になる）。
pub struct SubmitLessonReview<L, R, A, C> {
    lessons: L,
    reviews: R,
    analytics: A,
    clock: C,
}

impl<L, R, A, C> SubmitLessonReview<L, R, A, C>
where
    L: CompletedLessonLookup,
    R: LessonReviewRepository,
    A: AnalyticsSink,
    C: Clock,
{
    pub fn new(lessons: L, reviews: R, analytics: A, clock: C) -> Self {
        Self {
            lessons,
            reviews,
            analytics,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: SubmitLessonReviewCommand,
    ) -> Result<LessonReview, SubmitLessonReviewError> {
        let now = self.clock.now();
        let lesson = self
            .lessons
            .find_completion(command.lesson_id)
            .await
            .map_err(SubmitLessonReviewError::Unavailable)?
            .ok_or(SubmitLessonReviewError::LessonNotFound)?;
        let existing = self
            .reviews
            .find_by_lesson(command.lesson_id)
            .await
            .map_err(SubmitLessonReviewError::Unavailable)?;
        let review = LessonReview::submit(
            self.reviews.next_id(),
            command.account_id,
            &lesson,
            existing.as_ref(),
            command.rating,
            command.comment,
            now,
        )
        .map_err(SubmitLessonReviewError::Invalid)?;
        self.reviews
            .insert(review.clone())
            .await
            .map_err(|error| match error {
                InsertReviewError::LessonAlreadyReviewed => {
                    SubmitLessonReviewError::Invalid(DomainError::LessonAlreadyReviewed)
                }
                InsertReviewError::Unavailable(error) => {
                    SubmitLessonReviewError::Unavailable(error)
                }
            })?;

        self.analytics
            .emit(AnalyticsEvent::ReviewSubmitted {
                student_id: review.student_id(),
                coach_id: review.coach_id(),
                lesson_id: review.lesson_id(),
                review_id: review.id(),
                rating: review.rating(),
                occurred_at: now,
            })
            .await;

        Ok(review)
    }

    pub fn reviews(&self) -> &R {
        &self.reviews
    }

    pub fn analytics(&self) -> &A {
        &self.analytics
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::{ReviewComment, Video, VideoId, VideoReviewId},
        identity::Sport,
        lesson::LessonCompletion,
    };

    use super::*;
    use crate::{
        in_memory::{
            FixedClock, InMemoryLessonCompletionRepository, InMemoryLessonReviewRepository,
            RecordingAnalyticsSink,
        },
        lesson::{LessonCompletionLookup, LessonCompletionRepository},
    };

    type UseCase = SubmitLessonReview<
        LessonCompletionLookup<InMemoryLessonCompletionRepository>,
        InMemoryLessonReviewRepository,
        RecordingAnalyticsSink,
        FixedClock,
    >;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);

    fn at(hours: u64) -> SystemTime {
        // 2026-10-26T00:00:00Z から `hours` 時間後
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800 + hours * 3600)
    }

    /// 生徒 1 がコーチ 7 の添削を受け終えたレッスンを記録し、その完了記録を返す。
    async fn complete(
        completions: &InMemoryLessonCompletionRepository,
        id: u128,
    ) -> LessonCompletion {
        let mut video = Video::submit(VideoId::from_u128(id), STUDENT, COACH, Sport::Tennis, at(0));
        video
            .return_review(
                VideoReviewId::from_u128(id),
                COACH,
                ReviewComment::new("よい").unwrap(),
                true,
                at(1),
            )
            .unwrap();
        let lesson_id = completions.next_id();
        completions
            .record(STUDENT, |history| {
                video.complete(lesson_id, STUDENT, history, at(2))
            })
            .await
            .unwrap()
    }

    async fn use_case() -> (UseCase, LessonCompletion) {
        let completions = InMemoryLessonCompletionRepository::default();
        let lesson = complete(&completions, 40).await;
        let use_case = SubmitLessonReview::new(
            LessonCompletionLookup::new(completions),
            InMemoryLessonReviewRepository::default(),
            RecordingAnalyticsSink::default(),
            FixedClock(at(3)),
        );
        (use_case, lesson)
    }

    fn command(
        account_id: AccountId,
        lesson_id: LessonId,
        rating: i64,
    ) -> SubmitLessonReviewCommand {
        SubmitLessonReviewCommand {
            account_id,
            lesson_id,
            rating: Rating::new(rating).unwrap(),
            comment: LessonReviewComment::new("分かりやすかった").unwrap(),
        }
    }

    #[tokio::test]
    async fn the_student_reviews_a_completed_lesson_and_it_emits_review_submitted() {
        let (use_case, lesson) = use_case().await;

        let review = use_case
            .execute(command(STUDENT, lesson.id(), 4))
            .await
            .unwrap();

        assert_eq!(review.lesson_id(), lesson.id());
        assert_eq!((review.student_id(), review.coach_id()), (STUDENT, COACH));
        assert_eq!(review.rating().value(), 4);
        assert_eq!(review.comment().as_str(), "分かりやすかった");
        assert_eq!(review.submitted_at(), at(3));
        assert_eq!(use_case.reviews().all(), vec![review.clone()]);

        let analytics = use_case.analytics().events();
        assert_eq!(
            analytics,
            vec![AnalyticsEvent::ReviewSubmitted {
                student_id: STUDENT,
                coach_id: COACH,
                lesson_id: lesson.id(),
                review_id: review.id(),
                rating: Rating::new(4).unwrap(),
                occurred_at: at(3),
            }]
        );
        assert_eq!(analytics[0].name(), "review.submitted");
    }

    #[tokio::test]
    async fn a_lesson_is_reviewed_only_once() {
        let (use_case, lesson) = use_case().await;
        use_case
            .execute(command(STUDENT, lesson.id(), 5))
            .await
            .unwrap();

        let result = use_case.execute(command(STUDENT, lesson.id(), 1)).await;
        assert!(
            matches!(
                result,
                Err(SubmitLessonReviewError::Invalid(
                    DomainError::LessonAlreadyReviewed
                ))
            ),
            "{result:?}"
        );
        assert_eq!(use_case.reviews().all().len(), 1);
        assert_eq!(use_case.analytics().events().len(), 1);
    }

    #[tokio::test]
    async fn a_concurrent_second_review_is_rejected_by_the_repository() {
        let (use_case, lesson) = use_case().await;
        // 先に判定を通った別の評価が保存先に入った（並行な評価の後の方）
        let earlier = LessonReview::submit(
            use_case.reviews().next_id(),
            STUDENT,
            &lesson,
            None,
            Rating::new(3).unwrap(),
            LessonReviewComment::new("先").unwrap(),
            at(3),
        )
        .unwrap();
        use_case.reviews().insert(earlier).await.unwrap();
        let review = LessonReview::submit(
            use_case.reviews().next_id(),
            STUDENT,
            &lesson,
            None,
            Rating::new(5).unwrap(),
            LessonReviewComment::new("後").unwrap(),
            at(3),
        )
        .unwrap();

        assert!(matches!(
            use_case.reviews().insert(review).await,
            Err(InsertReviewError::LessonAlreadyReviewed)
        ));
    }

    #[tokio::test]
    async fn rejected_reviews_save_nothing_and_emit_nothing() {
        let (use_case, lesson) = use_case().await;

        for account in [OTHER_STUDENT, COACH] {
            let result = use_case.execute(command(account, lesson.id(), 4)).await;
            assert!(
                matches!(
                    result,
                    Err(SubmitLessonReviewError::Invalid(
                        DomainError::NotTheLessonStudent
                    ))
                ),
                "{result:?}"
            );
        }
        let result = use_case
            .execute(command(STUDENT, LessonId::from_u128(99), 4))
            .await;
        assert!(
            matches!(result, Err(SubmitLessonReviewError::LessonNotFound)),
            "{result:?}"
        );
        assert!(use_case.reviews().all().is_empty());
        assert!(use_case.analytics().events().is_empty());
    }
}
