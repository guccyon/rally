use std::time::SystemTime;

use super::{LessonReview, LowRatedReviewCriteria};
use crate::identity::AccountId;

/// コーチ 1 人の直近の低い評価の件数と、そのうち本部がまだ対応済みの印（[`super::LowRatedReviewHandling`]）を付けて
/// いない件数（admin-web の `getCoachOverview` の `low_rated_review_count` / `unhandled_low_rated_review_count`）。
///
/// 低い評価かどうか（しきい値と期間）は [`LowRatedReviewCriteria::matches`] で決め、ここでは数えるだけ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachLowRatedReviewCount {
    coach_id: AccountId,
    low_rated_count: u32,
    unhandled_count: u32,
}

impl CoachLowRatedReviewCount {
    /// `reviews` のうち、コーチ `coach_id` への評価で `now` に照会したときの低い評価を数える（他のコーチへの評価は
    /// 数えない）。
    pub fn tally(coach_id: AccountId, reviews: &[LessonReview], now: SystemTime) -> Self {
        let (low_rated_count, unhandled_count) = reviews
            .iter()
            .filter(|review| {
                review.coach_id() == coach_id && LowRatedReviewCriteria::matches(review, now)
            })
            .fold((0u32, 0u32), |(low, unhandled), review| {
                (low + 1, unhandled + u32::from(review.handling().is_none()))
            });
        Self {
            coach_id,
            low_rated_count,
            unhandled_count,
        }
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    /// 直近の低い評価の件数（対応済みかを問わない）。
    pub const fn low_rated_count(&self) -> u32 {
        self.low_rated_count
    }

    /// 直近の低い評価のうち、本部がまだ対応済みの印を付けていない件数。
    pub const fn unhandled_count(&self) -> u32 {
        self.unhandled_count
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        lesson::LessonCompletionId,
        review::{LessonReviewComment, LessonReviewId, LowRatedReviewHandling, Rating},
    };

    const DAY: Duration = Duration::from_secs(24 * 3600);
    const COACH: AccountId = AccountId::from_u128(3);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);

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

    fn handled(review: LessonReview) -> LessonReview {
        review.with_handling(Some(LowRatedReviewHandling::new(
            AccountId::from_u128(9),
            now() - DAY,
        )))
    }

    #[test]
    fn no_reviews_is_zero() {
        let count = CoachLowRatedReviewCount::tally(COACH, &[], now());
        assert_eq!(count.coach_id(), COACH);
        assert_eq!((count.low_rated_count(), count.unhandled_count()), (0, 0));
    }

    #[test]
    fn counts_low_reviews_of_the_coach_within_the_window_and_those_not_handled() {
        let reviews = [
            review(1, COACH, 1, now() - DAY),
            handled(review(2, COACH, 2, now() - 2 * DAY)),
            // 30 日前ちょうどは含む
            review(3, COACH, 2, now() - 30 * DAY),
            // ★3 以上は数えない
            review(4, COACH, 3, now()),
            // 30 日より前は、印がなくても数えない
            review(5, COACH, 1, now() - 30 * DAY - Duration::from_secs(1)),
            // 他のコーチの評価は数えない
            review(6, OTHER_COACH, 1, now()),
        ];
        let count = CoachLowRatedReviewCount::tally(COACH, &reviews, now());
        assert_eq!(count.low_rated_count(), 3);
        assert_eq!(count.unhandled_count(), 2, "対応済みの印のある 1 件を除く");
    }

    #[test]
    fn all_handled_leaves_no_unhandled() {
        let reviews = [
            handled(review(1, COACH, 1, now())),
            handled(review(2, COACH, 2, now() - DAY)),
        ];
        let count = CoachLowRatedReviewCount::tally(COACH, &reviews, now());
        assert_eq!((count.low_rated_count(), count.unhandled_count()), (2, 0));
    }
}
