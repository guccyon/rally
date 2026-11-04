use std::{cmp::Reverse, collections::BTreeMap};

use domain::identity::AccountId;

use super::{
    CoachReviewList, LessonReviewRepository, ListCoachReviewsError, ListCoachReviewsQuery,
    ReviewedLessonMenuLookup,
};
use crate::scheduling::calendar_day;

/// コーチが受けた評価の一覧のユースケース（admin-web・student-app の `listCoachReviews`。本部と生徒と、自分の分だけの
/// コーチが呼ぶ）。
/// 照会だけで、何も保存しない。
///
/// コーチへの評価を、送った時刻の新しい順に返す。同じ時刻は評価の ID の順。評価がなければ空。
/// 条件（[`ListCoachReviewsQuery`]）で絞っても並べ方は変えない。点の上限を指定されたら、点がそれ以下の評価（domain の
/// [`domain::review::LessonReview::is_rated_at_most`]）だけを残す。日付を両方省けば期間で絞らない。どちらかがあれば、
/// 送った日付（日本時間。[`calendar_day`] で区切る）が `from` から `to` まで（両端を含む）の評価（domain の
/// [`domain::review::LessonReview::is_submitted_in`]）だけを残す。`from` だけなら `from` から、`to` だけなら `to` までの
/// 7 日で、期間の規則（`to` が `from` より前でない・31 日まで）も本部が送ったお知らせの一覧と同じ
/// （[`calendar_day::period_ending`] と domain の [`domain::scheduling::BookingPeriod`]）。点と期間は合わせて使える。
/// 各評価には、評価した完了したレッスンの元（予約・動画）が持つレッスンメニューの写しを
/// [`ReviewedLessonMenuLookup`] で引いて添える（メニューがなければ添えない）。
/// 誰が呼べるか（役割と、コーチは自分の分だけ）は境界（interface）が確かめ、ここは呼んだ人によって結果を変えない。
/// 評価した生徒を一覧に載せないのも境界（interface のレスポンスの形）。
pub struct ListCoachReviews<R, M> {
    reviews: R,
    lesson_menus: M,
}

impl<R, M> ListCoachReviews<R, M>
where
    R: LessonReviewRepository,
    M: ReviewedLessonMenuLookup,
{
    pub fn new(reviews: R, lesson_menus: M) -> Self {
        Self {
            reviews,
            lesson_menus,
        }
    }

    /// 期間を作る前に保存先を読まない（期間の違反は読まずに返す）。
    pub async fn execute(
        &self,
        coach_id: AccountId,
        query: ListCoachReviewsQuery,
    ) -> Result<CoachReviewList, ListCoachReviewsError> {
        let period = match (query.from, query.to) {
            (None, None) => None,
            (from, to) => {
                let (_, _, period) = calendar_day::period_ending(from, to, || {
                    unreachable!("from か to のどちらかがあるので今日は読まない")
                })?;
                Some(period)
            }
        };
        let mut reviews = self.reviews.find_by_coach(coach_id).await?;
        if let Some(max_rating) = query.max_rating {
            reviews.retain(|review| review.is_rated_at_most(max_rating));
        }
        if let Some(period) = period {
            reviews.retain(|review| review.is_submitted_in(&period));
        }
        reviews.sort_by_key(|review| (Reverse(review.submitted_at()), review.id()));
        let mut lesson_menus = BTreeMap::new();
        for review in &reviews {
            if let Some(menu) = self
                .lesson_menus
                .find_lesson_menu(review.lesson_id())
                .await?
            {
                lesson_menus.insert(review.lesson_id(), menu);
            }
        }
        Ok(CoachReviewList {
            reviews,
            lesson_menus,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::{LessonMenuId, LessonMenuName},
        lesson::LessonCompletionId,
        review::{LessonReview, LessonReviewComment, LessonReviewId, ReviewLessonMenu},
    };

    use domain::{review::Rating, scheduling::SchedulingError};
    use time::{Date, Month};

    use super::*;
    use crate::{Unavailable, in_memory::InMemoryLessonReviewRepository};

    const COACH: AccountId = AccountId::from_u128(3);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);
    const NO_REVIEWS_COACH: AccountId = AccountId::from_u128(5);

    /// レッスン 1 と 2 だけがメニュー（ID はレッスンと同じ数）を持つ。
    struct FixedLessonMenus;

    impl ReviewedLessonMenuLookup for FixedLessonMenus {
        async fn find_lesson_menu(
            &self,
            lesson_id: LessonCompletionId,
        ) -> Result<Option<ReviewLessonMenu>, Unavailable> {
            Ok([1, 2]
                .contains(&lesson_id.as_u128())
                .then(|| menu(lesson_id.as_u128())))
        }
    }

    fn menu(id: u128) -> ReviewLessonMenu {
        ReviewLessonMenu::new(
            LessonMenuId::from_u128(id),
            LessonMenuName::new(format!("メニュー{id}")).unwrap(),
        )
    }

    fn review(id: u128, coach_id: AccountId, submitted_secs: u64) -> LessonReview {
        rated_review(id, coach_id, 4, submitted_secs)
    }

    fn rated_review(
        id: u128,
        coach_id: AccountId,
        rating: i64,
        submitted_secs: u64,
    ) -> LessonReview {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonCompletionId::from_u128(id),
            AccountId::from_u128(100 + id),
            coach_id,
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("よい").unwrap(),
            SystemTime::UNIX_EPOCH + Duration::from_secs(submitted_secs),
        )
    }

    fn at_most(max_rating: i64) -> ListCoachReviewsQuery {
        ListCoachReviewsQuery {
            max_rating: Some(Rating::new(max_rating).unwrap()),
            ..ListCoachReviewsQuery::default()
        }
    }

    fn use_case() -> ListCoachReviews<InMemoryLessonReviewRepository, FixedLessonMenus> {
        ListCoachReviews::new(
            InMemoryLessonReviewRepository::with_reviews([
                review(1, COACH, 10),
                review(2, COACH, 30),
                review(3, OTHER_COACH, 40),
                // 同じ時刻は ID の順
                review(5, COACH, 20),
                review(4, COACH, 20),
            ]),
            FixedLessonMenus,
        )
    }

    #[tokio::test]
    async fn lists_only_the_reviews_of_the_coach_newest_first() {
        let use_case = use_case();

        let list = use_case
            .execute(COACH, ListCoachReviewsQuery::default())
            .await
            .unwrap();
        let ids: Vec<LessonReviewId> = list.reviews.iter().map(LessonReview::id).collect();
        assert_eq!(
            ids,
            [2, 4, 5, 1].map(LessonReviewId::from_u128),
            "新しい順で、他のコーチの評価を含まない"
        );

        let empty = use_case
            .execute(NO_REVIEWS_COACH, ListCoachReviewsQuery::default())
            .await
            .unwrap();
        assert!(empty.reviews.is_empty());
        assert!(empty.lesson_menus.is_empty());
    }

    #[tokio::test]
    async fn adds_the_lesson_menu_of_each_reviewed_lesson_that_has_one() {
        let list = use_case()
            .execute(COACH, ListCoachReviewsQuery::default())
            .await
            .unwrap();
        // メニューのないレッスン（4・5）は含まない
        assert_eq!(
            list.lesson_menus,
            BTreeMap::from([
                (LessonCompletionId::from_u128(1), menu(1)),
                (LessonCompletionId::from_u128(2), menu(2)),
            ])
        );
    }

    #[tokio::test]
    async fn a_max_rating_keeps_only_reviews_rated_at_most_it_newest_first() {
        let use_case = ListCoachReviews::new(
            InMemoryLessonReviewRepository::with_reviews([
                rated_review(1, COACH, 2, 10),
                rated_review(2, COACH, 5, 30),
                rated_review(3, OTHER_COACH, 1, 40),
                rated_review(4, COACH, 3, 20),
                // 同じ時刻は ID の順
                rated_review(6, COACH, 1, 20),
                rated_review(5, COACH, 2, 20),
            ]),
            FixedLessonMenus,
        );
        let ids = |list: CoachReviewList| -> Vec<u128> {
            list.reviews.iter().map(|r| r.id().as_u128()).collect()
        };

        let list = use_case.execute(COACH, at_most(2)).await.unwrap();
        assert_eq!(
            list.lesson_menus,
            BTreeMap::from([(LessonCompletionId::from_u128(1), menu(1))]),
            "残した評価のメニューだけ"
        );
        assert_eq!(ids(list), [5, 6, 1], "その点以下だけを、新しい順で");
        assert_eq!(
            ids(use_case.execute(COACH, at_most(3)).await.unwrap()),
            [4, 5, 6, 1],
            "その点ちょうどを含む"
        );
        assert_eq!(
            ids(use_case.execute(COACH, at_most(5)).await.unwrap()),
            [2, 4, 5, 6, 1],
            "5 以下はすべて"
        );
        assert!(
            ids(use_case
                .execute(NO_REVIEWS_COACH, at_most(1))
                .await
                .unwrap())
            .is_empty()
        );
    }

    /// 2026-11-01T00:00:00+09:00（2026-10-31T15:00:00Z）の UNIX 秒
    const NOV1_JST: u64 = 1_793_458_800;
    const DAY: u64 = 24 * 60 * 60;

    fn date(month: Month, day: u8) -> Date {
        Date::from_calendar_date(2026, month, day).unwrap()
    }

    fn between(from: Option<Date>, to: Option<Date>) -> ListCoachReviewsQuery {
        ListCoachReviewsQuery {
            from,
            to,
            ..ListCoachReviewsQuery::default()
        }
    }

    /// 日本時間の日付の境目の前後に付いた評価。
    fn period_use_case() -> ListCoachReviews<InMemoryLessonReviewRepository, FixedLessonMenus> {
        ListCoachReviews::new(
            InMemoryLessonReviewRepository::with_reviews([
                // 10 月 31 日の終わり（日本時間）
                rated_review(1, COACH, 2, NOV1_JST - 1),
                // 11 月 1 日の始まり
                rated_review(2, COACH, 5, NOV1_JST),
                // 11 月 3 日の終わり
                rated_review(3, COACH, 1, NOV1_JST + 3 * DAY - 1),
                // 11 月 4 日の始まり
                rated_review(4, COACH, 2, NOV1_JST + 3 * DAY),
                // 11 月 7 日の終わりと 11 月 8 日の始まり
                rated_review(5, COACH, 3, NOV1_JST + 7 * DAY - 1),
                rated_review(6, COACH, 3, NOV1_JST + 7 * DAY),
                // 他のコーチの評価は期間に入っても載らない
                rated_review(7, OTHER_COACH, 1, NOV1_JST + DAY),
            ]),
            FixedLessonMenus,
        )
    }

    fn ids(list: CoachReviewList) -> Vec<u128> {
        list.reviews.iter().map(|r| r.id().as_u128()).collect()
    }

    #[tokio::test]
    async fn a_period_keeps_only_reviews_submitted_from_the_start_of_from_to_the_end_of_to_in_japan()
     {
        let use_case = period_use_case();
        let list = use_case
            .execute(
                COACH,
                between(
                    Some(date(Month::November, 1)),
                    Some(date(Month::November, 3)),
                ),
            )
            .await
            .unwrap();
        assert_eq!(
            list.lesson_menus,
            BTreeMap::from([(LessonCompletionId::from_u128(2), menu(2))]),
            "残した評価のメニューだけ"
        );
        assert_eq!(
            ids(list),
            [3, 2],
            "from の 0 時を含み、to の翌日の 0 時を含まない。新しい順"
        );

        let one_day = use_case
            .execute(
                COACH,
                between(
                    Some(date(Month::October, 31)),
                    Some(date(Month::October, 31)),
                ),
            )
            .await
            .unwrap();
        assert_eq!(ids(one_day), [1], "from と to が同じ日なら 1 日");

        let empty = use_case
            .execute(
                NO_REVIEWS_COACH,
                between(
                    Some(date(Month::November, 1)),
                    Some(date(Month::November, 3)),
                ),
            )
            .await
            .unwrap();
        assert!(empty.reviews.is_empty());
    }

    #[tokio::test]
    async fn only_from_or_only_to_makes_a_period_of_seven_days() {
        let use_case = period_use_case();
        let from_only = use_case
            .execute(COACH, between(Some(date(Month::November, 1)), None))
            .await
            .unwrap();
        assert_eq!(
            ids(from_only),
            [5, 4, 3, 2],
            "from から 7 日目（11 月 7 日）まで"
        );

        let to_only = use_case
            .execute(COACH, between(None, Some(date(Month::November, 7))))
            .await
            .unwrap();
        assert_eq!(ids(to_only), [5, 4, 3, 2], "to の 6 日前（11 月 1 日）から");
    }

    #[tokio::test]
    async fn a_period_and_a_max_rating_are_combined() {
        let list = period_use_case()
            .execute(
                COACH,
                ListCoachReviewsQuery {
                    max_rating: Some(Rating::new(2).unwrap()),
                    from: Some(date(Month::November, 1)),
                    to: Some(date(Month::November, 7)),
                },
            )
            .await
            .unwrap();
        assert_eq!(ids(list), [4, 3], "期間に入り、★2 以下のものだけ");
    }

    #[tokio::test]
    async fn a_period_that_breaks_the_rules_is_invalid() {
        let use_case = period_use_case();
        for (from, to, expected) in [
            (
                date(Month::November, 2),
                date(Month::November, 1),
                SchedulingError::BookingPeriodEndsBeforeItStarts,
            ),
            (
                date(Month::November, 1),
                date(Month::December, 2),
                SchedulingError::BookingPeriodTooLong { max_days: 31 },
            ),
        ] {
            let error = use_case
                .execute(COACH, between(Some(from), Some(to)))
                .await
                .unwrap_err();
            assert!(
                matches!(&error, ListCoachReviewsError::Invalid(actual) if *actual == expected),
                "{from}〜{to}: {error:?}"
            );
        }
        // 31 日ちょうどは通る
        assert!(
            use_case
                .execute(
                    COACH,
                    between(
                        Some(date(Month::November, 1)),
                        Some(date(Month::December, 1)),
                    ),
                )
                .await
                .is_ok()
        );
    }
}
