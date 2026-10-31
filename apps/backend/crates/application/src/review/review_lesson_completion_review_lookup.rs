use std::collections::BTreeSet;

use domain::{identity::AccountId, lesson::LessonCompletionId};

use super::LessonReviewRepository;
use crate::{Unavailable, lesson::LessonCompletionReviewLookup};

/// 完了記録の [`LessonCompletionReviewLookup`] の review による実装。評価の保存先から完了したレッスンごとの評価を
/// 読むだけで、何も変えない。
///
/// 評価は完了したレッスン 1 件につき 1 つ（[`LessonReviewRepository::find_by_lesson`]）で、付けた生徒が問い合わせた
/// 生徒と同じものだけを評価済みとする。
#[derive(Debug, Clone)]
pub struct ReviewLessonCompletionReviewLookup<N> {
    reviews: N,
}

impl<N> ReviewLessonCompletionReviewLookup<N> {
    pub fn new(reviews: N) -> Self {
        Self { reviews }
    }
}

impl<N> LessonCompletionReviewLookup for ReviewLessonCompletionReviewLookup<N>
where
    N: LessonReviewRepository,
{
    async fn find_reviewed_lessons(
        &self,
        student_id: AccountId,
        lesson_ids: &BTreeSet<LessonCompletionId>,
    ) -> Result<BTreeSet<LessonCompletionId>, Unavailable> {
        let mut reviewed = BTreeSet::new();
        for &lesson_id in lesson_ids {
            if let Some(review) = self.reviews.find_by_lesson(lesson_id).await?
                && review.student_id() == student_id
            {
                reviewed.insert(lesson_id);
            }
        }
        Ok(reviewed)
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::review::{LessonReview, LessonReviewComment, LessonReviewId, Rating};

    use super::*;
    use crate::in_memory::InMemoryLessonReviewRepository;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);

    fn review(id: u128, lesson_id: u128, student_id: AccountId) -> LessonReview {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonCompletionId::from_u128(lesson_id),
            student_id,
            COACH,
            Rating::new(5).unwrap(),
            LessonReviewComment::new("よかった").unwrap(),
            SystemTime::UNIX_EPOCH,
        )
    }

    #[tokio::test]
    async fn finds_the_asked_lessons_the_student_reviewed() {
        let lookup = ReviewLessonCompletionReviewLookup::new(
            InMemoryLessonReviewRepository::with_reviews([
                review(1, 10, STUDENT),
                // 問い合わせていないレッスン
                review(2, 12, STUDENT),
                // 別の生徒が付けた評価
                review(3, 13, OTHER_STUDENT),
            ]),
        );
        let lesson_ids = [10, 11, 13].map(LessonCompletionId::from_u128).into();

        let reviewed = lookup
            .find_reviewed_lessons(STUDENT, &lesson_ids)
            .await
            .unwrap();

        assert_eq!(
            reviewed,
            BTreeSet::from([LessonCompletionId::from_u128(10)])
        );
    }
}
