use std::{
    cmp::Reverse,
    time::{Duration, SystemTime},
};

use super::{LessonReview, Rating};

/// 本部が横断で見る「低い評価」の規則（admin-web の `listLowRatedLessonReviews`。しきい値と期間の正本は
/// `specs/contracts/api/admin-web/openapi.yaml` の `listLowRatedLessonReviews`）。
///
/// 点が [`LowRatedReviewCriteria::MAX_RATING`] 以下で、照会した時刻の [`LowRatedReviewCriteria::WINDOW_DAYS`] 日前
/// （含む）から照会した時刻（含む）までに送られた評価を「低い評価」とする。しきい値と期間はこの型の 1 か所だけに置く。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LowRatedReviewCriteria;

impl LowRatedReviewCriteria {
    /// この点以下の評価を低い評価とする（★2 以下）
    pub const MAX_RATING: u8 = 2;
    /// 見る期間の日数（直近 30 日）
    pub const WINDOW_DAYS: u32 = 30;
    /// 見る期間の長さ（[`Self::WINDOW_DAYS`] 日）
    pub const WINDOW: Duration = Duration::from_secs(Self::WINDOW_DAYS as u64 * 24 * 3600);

    /// `now` に照会したときの期間の始まり（含む）。`now` から期間を引けない（時刻の範囲の外）ときは UNIX 時刻の起点。
    pub fn since(now: SystemTime) -> SystemTime {
        now.checked_sub(Self::WINDOW)
            .unwrap_or(SystemTime::UNIX_EPOCH)
    }

    /// 点 `rating` が低い点（[`Self::MAX_RATING`] 以下）か。期間は問わない（本部の「対応済み」の印を付けられるか。
    /// [`LessonReview::mark_handled`]）。
    pub fn is_low_rating(rating: Rating) -> bool {
        rating.value() <= Self::MAX_RATING
    }

    /// `review` が `now` に照会したときの低い評価か。
    pub fn matches(review: &LessonReview, now: SystemTime) -> bool {
        Self::is_low_rating(review.rating())
            && (Self::since(now)..=now).contains(&review.submitted_at())
    }

    /// `reviews` から `now` に照会したときの低い評価だけを選び、送った時刻の新しい順（同じ時刻は評価の ID の順）に並べる。
    pub fn select(reviews: Vec<LessonReview>, now: SystemTime) -> Vec<LessonReview> {
        let mut low: Vec<LessonReview> = reviews
            .into_iter()
            .filter(|review| Self::matches(review, now))
            .collect();
        low.sort_by_key(|review| (Reverse(review.submitted_at()), review.id()));
        low
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        identity::AccountId,
        lesson::LessonCompletionId,
        review::{LessonReviewComment, LessonReviewId},
    };

    const DAY: Duration = Duration::from_secs(24 * 3600);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    fn review(id: u128, rating: i64, submitted_at: SystemTime) -> LessonReview {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonCompletionId::from_u128(id),
            AccountId::from_u128(100 + id),
            AccountId::from_u128(3),
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("いまひとつ").unwrap(),
            submitted_at,
        )
    }

    #[test]
    fn the_threshold_is_two_stars_and_the_window_is_thirty_days() {
        assert_eq!(LowRatedReviewCriteria::MAX_RATING, 2);
        assert_eq!(LowRatedReviewCriteria::WINDOW_DAYS, 30);
        assert_eq!(LowRatedReviewCriteria::WINDOW, 30 * DAY);
        assert_eq!(LowRatedReviewCriteria::since(now()), now() - 30 * DAY);
    }

    #[test]
    fn ratings_of_two_or_less_are_low_and_three_or_more_are_not() {
        for (rating, low) in [(1, true), (2, true), (3, false), (4, false), (5, false)] {
            assert_eq!(
                LowRatedReviewCriteria::matches(&review(1, rating, now()), now()),
                low,
                "★{rating}"
            );
        }
    }

    #[test]
    fn a_low_rating_ignores_the_window() {
        for (rating, low) in [(1, true), (2, true), (3, false), (5, false)] {
            assert_eq!(
                LowRatedReviewCriteria::is_low_rating(Rating::new(rating).unwrap()),
                low,
                "★{rating}"
            );
        }
    }

    #[test]
    fn only_reviews_within_the_last_thirty_days_including_both_ends() {
        let since = now() - 30 * DAY;
        for (submitted_at, low, why) in [
            (since, true, "30 日前ちょうどは含む"),
            (
                since - Duration::from_secs(1),
                false,
                "30 日前より前は含まない",
            ),
            (now(), true, "照会した時刻ちょうどは含む"),
            (
                now() + Duration::from_secs(1),
                false,
                "照会した時刻より後は含まない",
            ),
        ] {
            assert_eq!(
                LowRatedReviewCriteria::matches(&review(1, 1, submitted_at), now()),
                low,
                "{why}"
            );
        }
    }

    #[test]
    fn select_keeps_low_reviews_newest_first_then_by_id() {
        let reviews = vec![
            review(1, 1, now() - 10 * DAY),
            review(2, 5, now() - DAY),
            review(3, 2, now() - 31 * DAY),
            // 同じ時刻は ID の順
            review(5, 2, now() - 2 * DAY),
            review(4, 1, now() - 2 * DAY),
        ];
        let ids: Vec<u128> = LowRatedReviewCriteria::select(reviews, now())
            .iter()
            .map(|review| review.id().as_u128())
            .collect();
        assert_eq!(ids, [4, 5, 1]);
    }
}
