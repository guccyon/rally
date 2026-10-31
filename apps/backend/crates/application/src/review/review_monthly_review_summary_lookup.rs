use std::collections::BTreeMap;

use domain::{
    entitlement::YearMonth,
    identity::AccountId,
    review::{CoachReviewSummary, LessonReview},
};

use super::LessonReviewRepository;
use crate::{Unavailable, entitlement::calendar_month, lesson::MonthlyReviewSummaryLookup};

/// [`MonthlyReviewSummaryLookup`] の review による実装。コーチへの評価を保存先から読み、送った時刻の暦月（日本時間）ごとに
/// domain の [`CoachReviewSummary::tally`] で数えるだけで、何も変えない（数え方は [`super::GetCoachReviewSummary`] と同じ）。
#[derive(Debug, Clone)]
pub struct ReviewMonthlyReviewSummaryLookup<N> {
    reviews: N,
}

impl<N> ReviewMonthlyReviewSummaryLookup<N> {
    pub fn new(reviews: N) -> Self {
        Self { reviews }
    }
}

impl<N> MonthlyReviewSummaryLookup for ReviewMonthlyReviewSummaryLookup<N>
where
    N: LessonReviewRepository,
{
    async fn find_monthly_review_summaries(
        &self,
        coach_id: AccountId,
        months: &[YearMonth],
    ) -> Result<BTreeMap<YearMonth, CoachReviewSummary>, Unavailable> {
        let reviews = self.reviews.find_by_coach(coach_id).await?;
        Ok(months
            .iter()
            .map(|&month| {
                let in_month: Vec<LessonReview> = reviews
                    .iter()
                    .filter(|review| calendar_month::of(review.submitted_at()) == month)
                    .cloned()
                    .collect();
                (month, CoachReviewSummary::tally(coach_id, &in_month))
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        lesson::LessonCompletionId,
        review::{LessonReviewComment, LessonReviewId, Rating},
    };

    use super::*;
    use crate::in_memory::InMemoryLessonReviewRepository;

    const COACH: AccountId = AccountId::from_u128(3);
    /// 2026-11-01T00:00:00+09:00（2026-10-31T15:00:00Z）
    const NOV1_JST: Duration = Duration::from_secs(1_793_458_800);

    fn review(id: u128, coach_id: AccountId, rating: i64, at: SystemTime) -> LessonReview {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonCompletionId::from_u128(id),
            AccountId::from_u128(100 + id),
            coach_id,
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("よい").unwrap(),
            at,
        )
    }

    #[tokio::test]
    async fn tallies_reviews_by_the_month_in_japan_they_were_submitted() {
        let nov1 = SystemTime::UNIX_EPOCH + NOV1_JST;
        let lookup =
            ReviewMonthlyReviewSummaryLookup::new(InMemoryLessonReviewRepository::with_reviews([
                review(1, COACH, 5, nov1),
                review(2, COACH, 4, nov1 - Duration::from_secs(1)),
                review(3, COACH, 2, nov1 - Duration::from_secs(2)),
                review(4, AccountId::from_u128(4), 1, nov1),
            ]));
        let (oct, nov, dec) = (
            YearMonth::new(2026, 10).unwrap(),
            YearMonth::new(2026, 11).unwrap(),
            YearMonth::new(2026, 12).unwrap(),
        );

        let found = lookup
            .find_monthly_review_summaries(COACH, &[oct, nov, dec])
            .await
            .unwrap();
        let rows: Vec<_> = found
            .iter()
            .map(|(month, s)| (month.month(), s.review_count(), s.average_rating()))
            .collect();
        assert_eq!(
            rows,
            [(10, 2, Some(3.0)), (11, 1, Some(5.0)), (12, 0, None)],
            "日本時間の 11 月 1 日 0 時から 11 月。他のコーチへの評価は数えない。評価のない月は 0 件"
        );
    }
}
