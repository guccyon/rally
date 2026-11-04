use std::time::SystemTime;

use domain::{
    identity::AccountId,
    review::{LowRatedReviewCriteria, StudentLowRatedReviewCount},
};

use super::LessonReviewRepository;
use crate::{Unavailable, identity::StudentLowRatedReviewCountLookup};

/// identity の [`StudentLowRatedReviewCountLookup`] の review による実装。評価の保存先から期間の始まり（domain の
/// [`LowRatedReviewCriteria::since`]）以降に送られた評価を読み（[`LessonReviewRepository::find_submitted_since`]。
/// [`super::ListLowRatedReviews`] と同じ読み方）、照会した時刻で domain の [`StudentLowRatedReviewCount::tally`] で数える
/// だけで、何も変えない。
#[derive(Debug, Clone)]
pub struct ReviewStudentLowRatedReviewCountLookup<R> {
    reviews: R,
}

impl<R> ReviewStudentLowRatedReviewCountLookup<R> {
    pub fn new(reviews: R) -> Self {
        Self { reviews }
    }
}

impl<R> StudentLowRatedReviewCountLookup for ReviewStudentLowRatedReviewCountLookup<R>
where
    R: LessonReviewRepository,
{
    async fn find_low_rated_review_count(
        &self,
        student_id: AccountId,
        now: SystemTime,
    ) -> Result<StudentLowRatedReviewCount, Unavailable> {
        let reviews = self
            .reviews
            .find_submitted_since(LowRatedReviewCriteria::since(now))
            .await?;
        Ok(StudentLowRatedReviewCount::tally(student_id, &reviews, now))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        time::Duration,
    };

    use domain::{
        catalog::Sport,
        lesson::LessonCompletionId,
        profile::DisplayName,
        review::{
            LessonReview, LessonReviewComment, LessonReviewId, LowRatedReviewHandling, Rating,
            ReviewLessonMenu,
        },
    };

    use super::*;
    use crate::{
        in_memory::{FixedClock, InMemoryLessonReviewRepository},
        review::{
            ListLowRatedReviews, ListLowRatedReviewsQuery, ReviewedCoachDisplayNameLookup,
            ReviewedLessonMenuLookup, ReviewedLessonSportLookup, ReviewerDisplayNameLookup,
        },
    };

    const DAY: Duration = Duration::from_secs(24 * 3600);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const NO_REVIEWS_STUDENT: AccountId = AccountId::from_u128(5);
    const COACH: AccountId = AccountId::from_u128(3);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    fn review(
        id: u128,
        student_id: AccountId,
        coach_id: AccountId,
        rating: i64,
        at: SystemTime,
    ) -> LessonReview {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonCompletionId::from_u128(id),
            student_id,
            coach_id,
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("いまひとつ").unwrap(),
            at,
        )
    }

    fn reviews() -> InMemoryLessonReviewRepository {
        InMemoryLessonReviewRepository::with_reviews([
            review(1, STUDENT, COACH, 1, now()),
            review(2, STUDENT, OTHER_COACH, 2, now() - 30 * DAY).with_handling(Some(
                LowRatedReviewHandling::new(AccountId::from_u128(9), now()),
            )),
            review(3, STUDENT, COACH, 2, now() - 31 * DAY),
            review(4, STUDENT, COACH, 3, now()),
            review(5, OTHER_STUDENT, COACH, 2, now() - DAY),
        ])
    }

    /// 一覧に添えるものを何も知らない（数には要らない）。
    struct Nothing;

    impl ReviewedLessonMenuLookup for Nothing {
        async fn find_lesson_menu(
            &self,
            _: LessonCompletionId,
        ) -> Result<Option<ReviewLessonMenu>, Unavailable> {
            Ok(None)
        }
    }

    impl ReviewedLessonSportLookup for Nothing {
        async fn find_lesson_sport(
            &self,
            _: LessonCompletionId,
        ) -> Result<Option<Sport>, Unavailable> {
            Ok(None)
        }
    }

    impl ReviewedCoachDisplayNameLookup for Nothing {
        async fn find_coach_display_names(
            &self,
            _: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            Ok(BTreeMap::new())
        }
    }

    impl ReviewerDisplayNameLookup for Nothing {
        async fn find_student_display_names(
            &self,
            _: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            Ok(BTreeMap::new())
        }
    }

    #[tokio::test]
    async fn counts_low_reviews_by_the_student_at_the_time_of_the_query() {
        let lookup = ReviewStudentLowRatedReviewCountLookup::new(reviews());
        let count = lookup
            .find_low_rated_review_count(STUDENT, now())
            .await
            .unwrap();
        assert_eq!(count.student_id(), STUDENT);
        assert_eq!(
            (count.low_rated_count(), count.unhandled_count()),
            (2, 1),
            "30 日前ちょうどは含み、31 日前と ★3・他の生徒の評価は数えない。対応済みは未対応に数えない"
        );

        // 期間は照会した時刻から数える（1 日後には 30 日前ちょうどだった対応済みの評価が外れる）
        let later = lookup
            .find_low_rated_review_count(STUDENT, now() + DAY)
            .await
            .unwrap();
        assert_eq!((later.low_rated_count(), later.unhandled_count()), (1, 1));

        let none = lookup
            .find_low_rated_review_count(NO_REVIEWS_STUDENT, now())
            .await
            .unwrap();
        assert_eq!((none.low_rated_count(), none.unhandled_count()), (0, 0));
    }

    #[tokio::test]
    async fn the_count_is_the_same_as_the_low_rated_list_of_the_student() {
        let lookup = ReviewStudentLowRatedReviewCountLookup::new(reviews());
        for at in [now(), now() + DAY, now() - 2 * DAY] {
            for student_id in [STUDENT, OTHER_STUDENT, COACH, NO_REVIEWS_STUDENT] {
                let list = ListLowRatedReviews::new(
                    reviews(),
                    Nothing,
                    Nothing,
                    Nothing,
                    Nothing,
                    FixedClock(at),
                );
                let mut listed = [0, 0];
                for (n, handled) in [None, Some(false)].into_iter().enumerate() {
                    listed[n] = list
                        .execute(ListLowRatedReviewsQuery {
                            student_id: Some(student_id),
                            handled,
                            ..Default::default()
                        })
                        .await
                        .unwrap()
                        .reviews
                        .len();
                }
                let [listed, unhandled] = listed;
                let count = lookup
                    .find_low_rated_review_count(student_id, at)
                    .await
                    .unwrap();
                assert_eq!(
                    (
                        count.low_rated_count() as usize,
                        count.unhandled_count() as usize
                    ),
                    (listed, unhandled),
                    "{student_id:?} {at:?}"
                );
            }
        }
    }
}
