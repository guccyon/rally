use super::LessonReview;
use crate::identity::AccountId;

/// コーチ 1 人の評価の件数と平均（admin-web の `getCoachReviewSummary`。丸め方の正本は
/// `specs/contracts/api/admin-web/openapi.yaml` の `getCoachReviewSummary`）。
///
/// 数え方と平均の計算はこの型の 1 か所だけに置く。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachReviewSummary {
    coach_id: AccountId,
    review_count: u32,
    rating_total: u32,
}

impl CoachReviewSummary {
    /// `reviews` のうち、コーチ `coach_id` への評価を数える（他のコーチへの評価は数えない）。
    pub fn tally(coach_id: AccountId, reviews: &[LessonReview]) -> Self {
        let (review_count, rating_total) = reviews
            .iter()
            .filter(|review| review.coach_id() == coach_id)
            .fold((0u32, 0u32), |(count, total), review| {
                (count + 1, total + u32::from(review.rating().value()))
            });
        Self {
            coach_id,
            review_count,
            rating_total,
        }
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    /// 評価の件数。
    pub const fn review_count(&self) -> u32 {
        self.review_count
    }

    /// 評価の点の平均を、小数第 3 位で四捨五入して小数第 2 位までにした値。評価が 0 件なら `None`。
    pub fn average_rating(&self) -> Option<f64> {
        if self.review_count == 0 {
            return None;
        }
        // 整数のまま 100 倍して四捨五入し、浮動小数点の誤差を丸めに持ち込まない
        let (total, count) = (u64::from(self.rating_total), u64::from(self.review_count));
        let hundredths = (total * 200 + count) / (count * 2);
        Some(hundredths as f64 / 100.0)
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;
    use crate::{
        lesson::LessonCompletionId,
        review::{LessonReviewComment, LessonReviewId, Rating},
    };

    const COACH: AccountId = AccountId::from_u128(3);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);

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

    fn summary(ratings: &[i64]) -> CoachReviewSummary {
        let reviews: Vec<LessonReview> = ratings
            .iter()
            .enumerate()
            .map(|(i, rating)| review(i as u128, COACH, *rating))
            .collect();
        CoachReviewSummary::tally(COACH, &reviews)
    }

    #[test]
    fn no_reviews_has_no_average() {
        let summary = summary(&[]);
        assert_eq!(summary.coach_id(), COACH);
        assert_eq!(summary.review_count(), 0);
        assert_eq!(summary.average_rating(), None);
    }

    #[test]
    fn counts_and_averages_the_ratings_of_the_coach() {
        assert_eq!(summary(&[5]).average_rating(), Some(5.0));
        let summary = summary(&[4, 5]);
        assert_eq!(summary.review_count(), 2);
        assert_eq!(summary.average_rating(), Some(4.5));
    }

    #[test]
    fn rounds_the_average_half_up_to_two_decimal_places() {
        // 10 / 3 = 3.333… → 3.33、5 / 3 = 1.666… → 1.67、1.005 のような端は整数で丸めるので 1.01
        assert_eq!(summary(&[3, 3, 4]).average_rating(), Some(3.33));
        assert_eq!(summary(&[1, 1, 3]).average_rating(), Some(1.67));
        let mut ratings = vec![1; 199];
        ratings.push(2);
        // 201 / 200 = 1.005 → 1.01
        assert_eq!(summary(&ratings).average_rating(), Some(1.01));
    }

    #[test]
    fn reviews_of_other_coaches_do_not_count() {
        let reviews = [review(1, COACH, 2), review(2, OTHER_COACH, 5)];
        let summary = CoachReviewSummary::tally(COACH, &reviews);
        assert_eq!(summary.review_count(), 1);
        assert_eq!(summary.average_rating(), Some(2.0));
    }
}
