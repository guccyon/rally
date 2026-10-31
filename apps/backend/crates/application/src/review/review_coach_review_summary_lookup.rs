use std::collections::{BTreeMap, BTreeSet};

use domain::{identity::AccountId, review::CoachReviewSummary};

use super::LessonReviewRepository;
use crate::{Unavailable, matching::CoachReviewSummaryLookup};

/// matching の [`CoachReviewSummaryLookup`] の review による実装。評価の保存先から読み、domain の
/// [`CoachReviewSummary::tally`] で数えるだけで、何も変えない（数え方は [`super::GetCoachReviewSummary`] と同じ）。
#[derive(Debug, Clone)]
pub struct ReviewCoachReviewSummaryLookup<N> {
    reviews: N,
}

impl<N> ReviewCoachReviewSummaryLookup<N> {
    pub fn new(reviews: N) -> Self {
        Self { reviews }
    }
}

impl<N> CoachReviewSummaryLookup for ReviewCoachReviewSummaryLookup<N>
where
    N: LessonReviewRepository,
{
    async fn find_review_summaries(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, CoachReviewSummary>, Unavailable> {
        let mut summaries = BTreeMap::new();
        for &coach_id in coach_ids {
            let reviews = self.reviews.find_by_coach(coach_id).await?;
            summaries.insert(coach_id, CoachReviewSummary::tally(coach_id, &reviews));
        }
        Ok(summaries)
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

    fn review(id: u128, coach_id: u128, rating: i64) -> LessonReview {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonCompletionId::from_u128(id),
            AccountId::from_u128(100 + id),
            AccountId::from_u128(coach_id),
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("よい").unwrap(),
            SystemTime::UNIX_EPOCH,
        )
    }

    #[tokio::test]
    async fn tallies_each_coach_including_ones_without_reviews() {
        let lookup =
            ReviewCoachReviewSummaryLookup::new(InMemoryLessonReviewRepository::with_reviews([
                review(1, 3, 5),
                review(2, 3, 4),
                review(3, 4, 1),
            ]));

        let ids = [3, 5].map(AccountId::from_u128).into();
        let found = lookup.find_review_summaries(&ids).await.unwrap();
        let rows: Vec<_> = found
            .values()
            .map(|s| (s.coach_id().as_u128(), s.review_count(), s.average_rating()))
            .collect();
        assert_eq!(
            rows,
            [(3, 2, Some(4.5)), (5, 0, None)],
            "聞いていないコーチ（4）は含めない。評価のないコーチは 0 件"
        );
    }
}
