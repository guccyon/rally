use std::time::SystemTime;

use super::{LessonReview, LowRatedReviewCriteria};
use crate::identity::AccountId;

/// 生徒 1 人がつけた直近の低い評価の件数と、そのうち本部がまだ対応済みの印（[`super::LowRatedReviewHandling`]）を付けて
/// いない件数（admin-web の `getStudentActivity` の `low_rated_review_count` / `unhandled_low_rated_review_count`。正本は
/// `specs/contracts/api/admin-web/openapi.yaml` の `getStudentActivity`）。
///
/// `listLowRatedLessonReviews` にその生徒の `student_id` だけを付けたとき（`handled=false` も付けたとき）に載る評価の数と
/// 同じ。低い評価かどうか（しきい値と期間）は [`LowRatedReviewCriteria::matches`]、生徒で絞る規則は
/// [`LessonReview::matches_student`] の 1 か所だけで、ここでは数えるだけ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudentLowRatedReviewCount {
    student_id: AccountId,
    low_rated_count: u32,
    unhandled_count: u32,
}

impl StudentLowRatedReviewCount {
    /// `reviews` のうち、生徒 `student_id` がつけた評価で `now` に照会したときの低い評価を数える（他の生徒の評価は
    /// 数えない）。
    pub fn tally<'a>(
        student_id: AccountId,
        reviews: impl IntoIterator<Item = &'a LessonReview>,
        now: SystemTime,
    ) -> Self {
        let (low_rated_count, unhandled_count) = reviews
            .into_iter()
            .filter(|review| {
                review.matches_student(Some(student_id))
                    && LowRatedReviewCriteria::matches(review, now)
            })
            .fold((0u32, 0u32), |(low, unhandled), review| {
                (
                    low.saturating_add(1),
                    unhandled.saturating_add(u32::from(review.handling().is_none())),
                )
            });
        Self {
            student_id,
            low_rated_count,
            unhandled_count,
        }
    }

    pub const fn student_id(&self) -> AccountId {
        self.student_id
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
    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(3);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    fn review(
        id: u128,
        student_id: AccountId,
        coach_id: AccountId,
        rating: i64,
        submitted_at: SystemTime,
    ) -> LessonReview {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonCompletionId::from_u128(id),
            student_id,
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

    fn reviews() -> Vec<LessonReview> {
        vec![
            review(1, STUDENT, COACH, 1, now() - DAY),
            // 別のコーチへの評価も、その生徒がつけたものなら数える
            handled(review(
                2,
                STUDENT,
                AccountId::from_u128(4),
                2,
                now() - 2 * DAY,
            )),
            // 30 日前ちょうどは含む
            review(3, STUDENT, COACH, 2, now() - 30 * DAY),
            // ★3 以上は数えない
            review(4, STUDENT, COACH, 3, now()),
            // 30 日より前は、印がなくても数えない
            review(
                5,
                STUDENT,
                COACH,
                1,
                now() - 30 * DAY - Duration::from_secs(1),
            ),
            // 他の生徒の評価は数えない
            review(6, OTHER_STUDENT, COACH, 1, now()),
        ]
    }

    #[test]
    fn no_reviews_is_zero() {
        let count = StudentLowRatedReviewCount::tally(STUDENT, &[], now());
        assert_eq!(count.student_id(), STUDENT);
        assert_eq!((count.low_rated_count(), count.unhandled_count()), (0, 0));
    }

    #[test]
    fn counts_low_reviews_by_the_student_within_the_window_and_those_not_handled() {
        let count = StudentLowRatedReviewCount::tally(STUDENT, &reviews(), now());
        assert_eq!(count.low_rated_count(), 3);
        assert_eq!(count.unhandled_count(), 2, "対応済みの印のある 1 件を除く");

        let other = StudentLowRatedReviewCount::tally(OTHER_STUDENT, &reviews(), now());
        assert_eq!((other.low_rated_count(), other.unhandled_count()), (1, 1));

        // コーチの ID では、評価された側でも数えない（評価した生徒として絞る）
        let coach = StudentLowRatedReviewCount::tally(COACH, &reviews(), now());
        assert_eq!((coach.low_rated_count(), coach.unhandled_count()), (0, 0));
    }

    #[test]
    fn the_count_is_the_same_as_the_low_rated_list_of_the_student() {
        for student_id in [STUDENT, OTHER_STUDENT, COACH, AccountId::from_u128(99)] {
            for at in [now(), now() + DAY, now() - DAY, now() + 29 * DAY] {
                let listed: Vec<LessonReview> = LowRatedReviewCriteria::select(reviews(), at)
                    .into_iter()
                    .filter(|review| review.matches_student(Some(student_id)))
                    .collect();
                let count = StudentLowRatedReviewCount::tally(student_id, &reviews(), at);
                assert_eq!(
                    count.low_rated_count() as usize,
                    listed.len(),
                    "{student_id:?} {at:?}"
                );
                assert_eq!(
                    count.unhandled_count() as usize,
                    listed
                        .iter()
                        .filter(|review| review.handling().is_none())
                        .count(),
                    "{student_id:?} {at:?}"
                );
            }
        }
    }
}
