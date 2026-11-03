use super::CoachLowRatedReviewCount;

/// 本部がコーチを「未対応の低い評価のあるコーチ」に絞るときの条件（admin-web の `listCoaches` の
/// `has_unhandled_low_rated`）。「未対応の低い評価がある」をどう決めるかはここだけに置く。
///
/// - `has_unhandled_low_rated` が `true`: 直近の低い評価のうち本部がまだ対応済みの印を付けていない件数
///   （[`CoachLowRatedReviewCount::unhandled_count`]。`getCoachOverview` の `unhandled_low_rated_review_count` と同じ数）が
///   1 件以上のコーチだけ
/// - `false`（既定）: 絞らない（どのコーチも通す）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnhandledLowRatedCoachFilter(bool);

impl UnhandledLowRatedCoachFilter {
    /// `has_unhandled_low_rated` が `true` なら未対応の低い評価のあるコーチだけに絞る条件。`false` なら絞らない。
    pub const fn new(has_unhandled_low_rated: bool) -> Self {
        Self(has_unhandled_low_rated)
    }

    /// 未対応の低い評価のあるコーチだけに絞るか。
    pub const fn has_unhandled_low_rated(&self) -> bool {
        self.0
    }

    /// 照会した時刻で数えた `low_rated_reviews` のコーチがこの条件に合うか。
    pub const fn matches(&self, low_rated_reviews: &CoachLowRatedReviewCount) -> bool {
        !self.0 || low_rated_reviews.unhandled_count() > 0
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::{
        identity::AccountId,
        lesson::LessonCompletionId,
        review::{
            LessonReview, LessonReviewComment, LessonReviewId, LowRatedReviewHandling, Rating,
        },
    };

    const COACH: AccountId = AccountId::from_u128(3);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);
    const DAY: Duration = Duration::from_secs(24 * 3600);

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

    fn tally(reviews: &[LessonReview]) -> CoachLowRatedReviewCount {
        CoachLowRatedReviewCount::tally(COACH, reviews, now())
    }

    #[test]
    fn has_unhandled_low_rated_matches_only_coaches_with_an_unhandled_low_review() {
        let filter = UnhandledLowRatedCoachFilter::new(true);
        assert!(filter.has_unhandled_low_rated());
        assert!(!filter.matches(&tally(&[])), "評価がなければ載らない");
        assert!(
            filter.matches(&tally(&[
                handled(review(1, COACH, 1, now())),
                // 30 日前ちょうどの ★2 は数える
                review(2, COACH, 2, now() - 30 * DAY),
            ])),
            "未対応の低い評価が 1 件でもあれば載る"
        );
        assert!(
            !filter.matches(&tally(&[
                // 対応済み・★3 以上・30 日より前・他のコーチへの評価は数えない
                handled(review(1, COACH, 1, now())),
                review(2, COACH, 3, now()),
                review(3, COACH, 1, now() - 30 * DAY - Duration::from_secs(1)),
                review(4, OTHER_COACH, 1, now()),
            ])),
            "数えない評価だけなら載らない"
        );
    }

    #[test]
    fn not_has_unhandled_low_rated_matches_every_coach() {
        let filter = UnhandledLowRatedCoachFilter::default();
        assert_eq!(filter, UnhandledLowRatedCoachFilter::new(false));
        assert!(!filter.has_unhandled_low_rated());
        assert!(filter.matches(&tally(&[])));
        assert!(filter.matches(&tally(&[review(1, COACH, 1, now())])));
    }
}
