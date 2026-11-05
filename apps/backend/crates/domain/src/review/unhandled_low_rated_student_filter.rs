use super::StudentLowRatedReviewCount;

/// 本部が生徒を「未対応の低い評価をつけた生徒」に絞るときの条件（admin-web の `listStudents` の
/// `has_unhandled_low_rated`）。「未対応の低い評価をつけた」をどう決めるかはここだけに置く。
///
/// - `has_unhandled_low_rated` が `true`: 生徒がつけた直近の低い評価のうち本部がまだ対応済みの印を付けていない件数
///   （[`StudentLowRatedReviewCount::unhandled_count`]。`getStudentActivity` の `unhandled_low_rated_review_count` と同じ数）が
///   1 件以上の生徒だけ
/// - `false`（既定）: 絞らない（どの生徒も通す）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnhandledLowRatedStudentFilter(bool);

impl UnhandledLowRatedStudentFilter {
    /// `has_unhandled_low_rated` が `true` なら未対応の低い評価をつけた生徒だけに絞る条件。`false` なら絞らない。
    pub const fn new(has_unhandled_low_rated: bool) -> Self {
        Self(has_unhandled_low_rated)
    }

    /// 未対応の低い評価をつけた生徒だけに絞るか。
    pub const fn has_unhandled_low_rated(&self) -> bool {
        self.0
    }

    /// 照会した時刻で数えた `low_rated_reviews` の生徒がこの条件に合うか。
    pub const fn matches(&self, low_rated_reviews: &StudentLowRatedReviewCount) -> bool {
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

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(3);
    const DAY: Duration = Duration::from_secs(24 * 3600);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    fn review(
        id: u128,
        student_id: AccountId,
        rating: i64,
        submitted_at: SystemTime,
    ) -> LessonReview {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonCompletionId::from_u128(id),
            student_id,
            COACH,
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

    fn tally(reviews: &[LessonReview]) -> StudentLowRatedReviewCount {
        StudentLowRatedReviewCount::tally(STUDENT, reviews, now())
    }

    #[test]
    fn has_unhandled_low_rated_matches_only_students_with_an_unhandled_low_review() {
        let filter = UnhandledLowRatedStudentFilter::new(true);
        assert!(filter.has_unhandled_low_rated());
        assert!(!filter.matches(&tally(&[])), "評価がなければ載らない");
        assert!(
            filter.matches(&tally(&[
                handled(review(1, STUDENT, 1, now())),
                // 30 日前ちょうどの ★2 は数える
                review(2, STUDENT, 2, now() - 30 * DAY),
            ])),
            "未対応の低い評価が 1 件でもあれば載る"
        );
        assert!(
            !filter.matches(&tally(&[
                // 対応済み・★3 以上・30 日より前・他の生徒の評価は数えない
                handled(review(1, STUDENT, 1, now())),
                review(2, STUDENT, 3, now()),
                review(3, STUDENT, 1, now() - 30 * DAY - Duration::from_secs(1)),
                review(4, OTHER_STUDENT, 1, now()),
            ])),
            "数えない評価だけなら載らない"
        );
    }

    #[test]
    fn has_unhandled_low_rated_is_the_same_as_the_unhandled_count_of_the_student_activity() {
        let reviews = [
            review(1, STUDENT, 1, now() - DAY),
            handled(review(2, OTHER_STUDENT, 2, now() - DAY)),
        ];
        let filter = UnhandledLowRatedStudentFilter::new(true);
        for student_id in [STUDENT, OTHER_STUDENT, COACH] {
            for at in [now(), now() - 2 * DAY, now() + 29 * DAY, now() + 30 * DAY] {
                let count = StudentLowRatedReviewCount::tally(student_id, &reviews, at);
                assert_eq!(
                    filter.matches(&count),
                    count.unhandled_count() >= 1,
                    "{student_id:?} {at:?}"
                );
            }
        }
        assert!(filter.matches(&StudentLowRatedReviewCount::tally(STUDENT, &reviews, now())));
        assert!(
            !filter.matches(&StudentLowRatedReviewCount::tally(
                OTHER_STUDENT,
                &reviews,
                now()
            )),
            "対応済みの低い評価だけなら載らない"
        );
    }

    #[test]
    fn not_has_unhandled_low_rated_matches_every_student() {
        let filter = UnhandledLowRatedStudentFilter::default();
        assert_eq!(filter, UnhandledLowRatedStudentFilter::new(false));
        assert!(!filter.has_unhandled_low_rated());
        assert!(filter.matches(&tally(&[])));
        assert!(filter.matches(&tally(&[review(1, STUDENT, 1, now())])));
    }
}
