use domain::{identity::AccountId, review::CoachLowRatedReviewCount};

use super::LessonReviewRepository;
use crate::{Unavailable, clock::Clock};

/// コーチ 1 人の直近の低い評価の件数と、そのうち本部が未対応の件数のユースケース（admin-web の `getCoachOverview` の
/// 項目。本部だけが呼ぶ）。照会だけで、何も保存しない。
///
/// コーチへの評価を読み、照会した時刻で domain の [`CoachLowRatedReviewCount::tally`] で数える（しきい値と期間は
/// domain の [`domain::review::LowRatedReviewCriteria`] だけ。`listLowRatedLessonReviews` と同じ線）。
/// 誰が呼べるか（本部だけ）は境界（interface）が確かめる。
pub struct GetCoachLowRatedReviewCount<R, C> {
    reviews: R,
    clock: C,
}

impl<R, C> GetCoachLowRatedReviewCount<R, C>
where
    R: LessonReviewRepository,
    C: Clock,
{
    pub fn new(reviews: R, clock: C) -> Self {
        Self { reviews, clock }
    }

    pub async fn execute(
        &self,
        coach_id: AccountId,
    ) -> Result<CoachLowRatedReviewCount, Unavailable> {
        let now = self.clock.now();
        let reviews = self.reviews.find_by_coach(coach_id).await?;
        Ok(CoachLowRatedReviewCount::tally(coach_id, &reviews, now))
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

    fn review(
        id: u128,
        coach_id: AccountId,
        rating: i64,
        submitted_at: SystemTime,
    ) -> LessonReview {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonCompletionId::from_u128(id),
            AccountId::from_u128(100 + id),
            coach_id,
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("いまひとつ").unwrap(),
            submitted_at,
        )
    }

    #[tokio::test]
    async fn counts_recent_low_reviews_of_the_coach_at_the_time_of_the_query() {
        let reviews = InMemoryLessonReviewRepository::with_reviews([
            review(1, COACH, 1, now() - DAY),
            review(2, COACH, 2, now() - 2 * DAY).with_handling(Some(LowRatedReviewHandling::new(
                AccountId::from_u128(9),
                now() - DAY,
            ))),
            review(3, COACH, 5, now()),
            review(4, COACH, 1, now() - 31 * DAY),
            review(5, OTHER_COACH, 1, now()),
        ]);

        let count = GetCoachLowRatedReviewCount::new(reviews.clone(), FixedClock(now()))
            .execute(COACH)
            .await
            .unwrap();
        assert_eq!(count.coach_id(), COACH);
        assert_eq!((count.low_rated_count(), count.unhandled_count()), (2, 1));

        // 期間は照会した時刻から数える
        let later = GetCoachLowRatedReviewCount::new(reviews.clone(), FixedClock(now() + 29 * DAY))
            .execute(COACH)
            .await
            .unwrap();
        assert_eq!(
            (later.low_rated_count(), later.unhandled_count()),
            (1, 1),
            "29 日後には 1 日前の評価だけが期間に残る（2 日前の対応済みの評価は出る）"
        );

        let empty = GetCoachLowRatedReviewCount::new(reviews, FixedClock(now()))
            .execute(NO_REVIEWS_COACH)
            .await
            .unwrap();
        assert_eq!((empty.low_rated_count(), empty.unhandled_count()), (0, 0));
    }
}
