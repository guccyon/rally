use domain::{identity::AccountId, review::CoachReviewSummary};

use super::LessonReviewRepository;
use crate::Unavailable;

/// コーチごとの評価の件数と平均のユースケース（admin-web・student-app の `getCoachReviewSummary`。本部と生徒と、
/// 自分の分だけのコーチが呼ぶ）。
///
/// コーチへの評価を読み、domain の [`CoachReviewSummary::tally`] で数える。数え方と平均の丸めは domain だけ。
/// 誰が呼べるか（役割と、コーチは自分の分だけ）は境界（interface）が確かめ、ここは呼んだ人によって結果を変えない。
pub struct GetCoachReviewSummary<R> {
    reviews: R,
}

impl<R> GetCoachReviewSummary<R>
where
    R: LessonReviewRepository,
{
    pub fn new(reviews: R) -> Self {
        Self { reviews }
    }

    pub async fn execute(&self, coach_id: AccountId) -> Result<CoachReviewSummary, Unavailable> {
        let reviews = self.reviews.find_by_coach(coach_id).await?;
        Ok(CoachReviewSummary::tally(coach_id, &reviews))
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::{
        lesson::LessonCompletionId,
        review::{LessonReview, LessonReviewComment, LessonReviewId, Rating},
    };

    use super::*;
    use crate::in_memory::InMemoryLessonReviewRepository;

    const COACH: AccountId = AccountId::from_u128(3);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);
    const NO_REVIEWS_COACH: AccountId = AccountId::from_u128(5);

    fn review(id: u128, coach_id: AccountId, rating: i64) -> LessonReview {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonCompletionId::from_u128(id),
            AccountId::from_u128(100 + id),
            coach_id,
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("よい").unwrap(),
            SystemTime::UNIX_EPOCH,
        )
    }

    #[tokio::test]
    async fn counts_only_the_reviews_of_the_coach() {
        let use_case = GetCoachReviewSummary::new(InMemoryLessonReviewRepository::with_reviews([
            review(1, COACH, 5),
            review(2, COACH, 4),
            review(3, OTHER_COACH, 1),
        ]));

        let summary = use_case.execute(COACH).await.unwrap();
        assert_eq!(summary.coach_id(), COACH);
        assert_eq!(summary.review_count(), 2);
        assert_eq!(summary.average_rating(), Some(4.5));

        let empty = use_case.execute(NO_REVIEWS_COACH).await.unwrap();
        assert_eq!(empty.review_count(), 0);
        assert_eq!(empty.average_rating(), None);
    }
}
