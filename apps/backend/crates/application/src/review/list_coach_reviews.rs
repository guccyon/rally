use std::cmp::Reverse;

use domain::{identity::AccountId, review::LessonReview};

use super::LessonReviewRepository;
use crate::Unavailable;

/// コーチが受けた評価の一覧のユースケース（admin-web の `listCoachReviews`。本部と、自分の分だけのコーチが呼ぶ）。
/// 照会だけで、何も保存しない。
///
/// コーチへの評価を、送った時刻の新しい順に返す。同じ時刻は評価の ID の順。評価がなければ空。
/// 誰が呼べるか（役割と、コーチは自分の分だけ）は境界（interface）が確かめる。
pub struct ListCoachReviews<R> {
    reviews: R,
}

impl<R> ListCoachReviews<R>
where
    R: LessonReviewRepository,
{
    pub fn new(reviews: R) -> Self {
        Self { reviews }
    }

    pub async fn execute(&self, coach_id: AccountId) -> Result<Vec<LessonReview>, Unavailable> {
        let mut reviews = self.reviews.find_by_coach(coach_id).await?;
        reviews.sort_by_key(|review| (Reverse(review.submitted_at()), review.id()));
        Ok(reviews)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        lesson::LessonId,
        review::{LessonReviewComment, LessonReviewId, Rating},
    };

    use super::*;
    use crate::in_memory::InMemoryLessonReviewRepository;

    const COACH: AccountId = AccountId::from_u128(3);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);
    const NO_REVIEWS_COACH: AccountId = AccountId::from_u128(5);

    fn review(id: u128, coach_id: AccountId, submitted_secs: u64) -> LessonReview {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonId::from_u128(id),
            AccountId::from_u128(100 + id),
            coach_id,
            Rating::new(4).unwrap(),
            LessonReviewComment::new("よい").unwrap(),
            SystemTime::UNIX_EPOCH + Duration::from_secs(submitted_secs),
        )
    }

    #[tokio::test]
    async fn lists_only_the_reviews_of_the_coach_newest_first() {
        let use_case = ListCoachReviews::new(InMemoryLessonReviewRepository::with_reviews([
            review(1, COACH, 10),
            review(2, COACH, 30),
            review(3, OTHER_COACH, 40),
            // 同じ時刻は ID の順
            review(5, COACH, 20),
            review(4, COACH, 20),
        ]));

        let reviews = use_case.execute(COACH).await.unwrap();
        let ids: Vec<LessonReviewId> = reviews.iter().map(LessonReview::id).collect();
        assert_eq!(
            ids,
            [2, 4, 5, 1].map(LessonReviewId::from_u128),
            "新しい順で、他のコーチの評価を含まない"
        );

        assert!(use_case.execute(NO_REVIEWS_COACH).await.unwrap().is_empty());
    }
}
