use std::collections::{BTreeMap, BTreeSet};

use domain::{identity::AccountId, review::CoachLowRatedReviewCount};

use super::LessonReviewRepository;
use crate::{Unavailable, clock::Clock, profile::CoachLowRatedReviewCountLookup};

/// profile の [`CoachLowRatedReviewCountLookup`] の review による実装。評価の保存先から読み、照会した時刻で domain の
/// [`CoachLowRatedReviewCount::tally`] で数えるだけで、何も変えない（数え方は [`super::GetCoachLowRatedReviewCount`] と
/// 同じ）。
#[derive(Debug, Clone)]
pub struct ReviewCoachLowRatedReviewCountLookup<R, C> {
    reviews: R,
    clock: C,
}

impl<R, C> ReviewCoachLowRatedReviewCountLookup<R, C> {
    pub fn new(reviews: R, clock: C) -> Self {
        Self { reviews, clock }
    }
}

impl<R, C> CoachLowRatedReviewCountLookup for ReviewCoachLowRatedReviewCountLookup<R, C>
where
    R: LessonReviewRepository,
    C: Clock,
{
    async fn find_low_rated_review_counts(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, CoachLowRatedReviewCount>, Unavailable> {
        let now = self.clock.now();
        let mut counts = BTreeMap::new();
        for &coach_id in coach_ids {
            let reviews = self.reviews.find_by_coach(coach_id).await?;
            counts.insert(
                coach_id,
                CoachLowRatedReviewCount::tally(coach_id, &reviews, now),
            );
        }
        Ok(counts)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        lesson::LessonCompletionId,
        review::{
            LessonReview, LessonReviewComment, LessonReviewId, LowRatedReviewHandling, Rating,
        },
    };

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryLessonReviewRepository};

    const DAY: Duration = Duration::from_secs(24 * 3600);
    const COACH: AccountId = AccountId::from_u128(3);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);
    const NO_REVIEWS_COACH: AccountId = AccountId::from_u128(5);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    fn review(id: u128, coach_id: AccountId, rating: i64, at: SystemTime) -> LessonReview {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonCompletionId::from_u128(id),
            AccountId::from_u128(100 + id),
            coach_id,
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("いまひとつ").unwrap(),
            at,
        )
    }

    #[tokio::test]
    async fn counts_low_reviews_of_each_coach_at_the_time_of_the_query() {
        let reviews = InMemoryLessonReviewRepository::with_reviews([
            review(1, COACH, 1, now()),
            review(2, COACH, 2, now() - 30 * DAY).with_handling(Some(LowRatedReviewHandling::new(
                AccountId::from_u128(9),
                now(),
            ))),
            review(3, COACH, 2, now() - 31 * DAY),
            review(4, COACH, 3, now()),
            review(5, OTHER_COACH, 2, now() - DAY),
        ]);
        let counts = ReviewCoachLowRatedReviewCountLookup::new(reviews.clone(), FixedClock(now()))
            .find_low_rated_review_counts(&BTreeSet::from([COACH, OTHER_COACH, NO_REVIEWS_COACH]))
            .await
            .unwrap();
        let rows: Vec<(AccountId, u32, u32)> = counts
            .iter()
            .map(|(&id, count)| (id, count.low_rated_count(), count.unhandled_count()))
            .collect();
        assert_eq!(
            rows,
            [(COACH, 2, 1), (OTHER_COACH, 1, 1), (NO_REVIEWS_COACH, 0, 0)],
            "30 日前ちょうどは含み、31 日前と ★3 は数えない。評価のないコーチも 0 件で含める"
        );

        // 期間は照会した時刻から数える
        let later =
            ReviewCoachLowRatedReviewCountLookup::new(reviews, FixedClock(now() + 30 * DAY))
                .find_low_rated_review_counts(&BTreeSet::from([COACH]))
                .await
                .unwrap();
        assert_eq!(later[&COACH].unhandled_count(), 1);
        assert_eq!(later[&COACH].low_rated_count(), 1);
    }
}
