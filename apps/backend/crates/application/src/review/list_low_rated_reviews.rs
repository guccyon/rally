use std::collections::{BTreeMap, BTreeSet};

use domain::{identity::AccountId, review::LowRatedReviewCriteria};

use super::{
    LessonReviewRepository, ListLowRatedReviewsError, ListLowRatedReviewsQuery, LowRatedReviewList,
    ReviewedCoachDisplayNameLookup, ReviewedLessonMenuLookup, ReviewedLessonSportLookup,
    ReviewerDisplayNameLookup,
};
use crate::{clock::Clock, scheduling::calendar_day};

/// 本部が見る低い評価の一覧のユースケース（admin-web の `listLowRatedLessonReviews`。本部だけが呼ぶ）。
/// 照会だけで、何も保存しない。
///
/// 期間の始まり（domain の [`LowRatedReviewCriteria::since`]）以降に送られた評価を読み、低い評価を domain の
/// [`LowRatedReviewCriteria::select`] で選んで並べる（しきい値と期間は domain だけ）。条件（[`ListLowRatedReviewsQuery`]）で
/// 本部の対応済みかを指定されたら、
/// 対応済みの印（domain の [`domain::review::LowRatedReviewHandling`]）の有無がそれと同じ評価だけを残す。コーチを指定されたら、
/// 評価されたコーチがそれと同じ評価だけを残す（いないコーチなら空の一覧。規則は domain の
/// [`LessonReview::matches_coach`](domain::review::LessonReview::matches_coach)）。生徒を指定されたら、評価した生徒が
/// それと同じ評価だけを残す（いない生徒なら空の一覧。規則は domain の
/// [`LessonReview::matches_student`](domain::review::LessonReview::matches_student)）。スポーツを指定されたら、評価した
/// 完了したレッスンのスポーツ（[`ReviewedLessonSportLookup`]。レッスンの元の枠・動画のスポーツ）がそれと同じ評価だけを残す
/// （スポーツが引けない評価は残さない）。日付 `from`・`to` のどちらかを指定されたら、送った日付（日本時間。
/// [`calendar_day`] で区切る）が `from` から `to` まで（両端を含む）の評価だけを残す（規則は domain の
/// [`LessonReview::matches_period`](domain::review::LessonReview::matches_period)）。直近の期間（30 日）の外は期間を
/// 指定しても載らない。`from` だけなら `from` から、`to` だけなら `to` までの 7 日で、期間の規則（`to` が `from` より前で
/// ない・31 日まで）は本部が送ったお知らせの一覧（[`crate::notification::ListAnnouncements`]）と同じ
/// [`calendar_day::period_ending`]・domain の [`domain::scheduling::BookingPeriod`]。各評価には、評価した完了した
/// レッスンの元が持つレッスンメニューの写し（[`ReviewedLessonMenuLookup`]）と、コーチ・生徒の表示名
/// （[`ReviewedCoachDisplayNameLookup`]・[`ReviewerDisplayNameLookup`]）を引いて添える。
/// 誰が呼べるか（本部だけ）は境界（interface）が確かめる。
pub struct ListLowRatedReviews<R, M, P, D, S, C> {
    reviews: R,
    lesson_menus: M,
    lesson_sports: P,
    coach_names: D,
    student_names: S,
    clock: C,
}

impl<R, M, P, D, S, C> ListLowRatedReviews<R, M, P, D, S, C>
where
    R: LessonReviewRepository,
    M: ReviewedLessonMenuLookup,
    P: ReviewedLessonSportLookup,
    D: ReviewedCoachDisplayNameLookup,
    S: ReviewerDisplayNameLookup,
    C: Clock,
{
    pub fn new(
        reviews: R,
        lesson_menus: M,
        lesson_sports: P,
        coach_names: D,
        student_names: S,
        clock: C,
    ) -> Self {
        Self {
            reviews,
            lesson_menus,
            lesson_sports,
            coach_names,
            student_names,
            clock,
        }
    }

    /// `query` の `sport` があれば、そのスポーツのレッスンの評価だけ。なければスポーツを問わない。`handled` があれば、本部の
    /// 対応済みの印があるか（`true`）・ないか（`false`）がそれと同じ評価だけ。なければ印を問わない。`coach_id` があれば、
    /// そのコーチが評価された評価だけ。なければコーチを問わない。`student_id` があれば、その生徒が評価した評価だけ。
    /// なければ生徒を問わない。`from`・`to` のどちらかがあれば、送った日付がその期間の
    /// 評価だけ。両方なければ期間で絞らない。期間が規則を満たさなければ [`ListLowRatedReviewsError::Invalid`]（評価は読まない）。
    pub async fn execute(
        &self,
        query: ListLowRatedReviewsQuery,
    ) -> Result<LowRatedReviewList, ListLowRatedReviewsError> {
        let ListLowRatedReviewsQuery {
            sport,
            handled,
            coach_id,
            student_id,
            from,
            to,
        } = query;
        let period = match (from, to) {
            (None, None) => None,
            (from, to) => {
                let (_, _, period) = calendar_day::period_ending(from, to, || {
                    unreachable!("from か to のどちらかがあるので今日は読まない")
                })?;
                Some(period)
            }
        };
        let now = self.clock.now();
        let submitted = self
            .reviews
            .find_submitted_since(LowRatedReviewCriteria::since(now))
            .await?;
        let mut reviews = LowRatedReviewCriteria::select(submitted, now);
        reviews.retain(|review| {
            review.matches_coach(coach_id)
                && review.matches_student(student_id)
                && review.matches_period(period.as_ref())
        });
        if let Some(handled) = handled {
            reviews.retain(|review| review.handling().is_some() == handled);
        }
        if let Some(sport) = sport {
            let mut of_sport = Vec::with_capacity(reviews.len());
            for review in reviews {
                if self
                    .lesson_sports
                    .find_lesson_sport(review.lesson_id())
                    .await?
                    == Some(sport)
                {
                    of_sport.push(review);
                }
            }
            reviews = of_sport;
        }
        if reviews.is_empty() {
            return Ok(LowRatedReviewList {
                reviews,
                lesson_menus: BTreeMap::new(),
                coach_display_names: BTreeMap::new(),
                student_display_names: BTreeMap::new(),
            });
        }

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
        let coach_ids: BTreeSet<AccountId> = reviews.iter().map(|r| r.coach_id()).collect();
        let student_ids: BTreeSet<AccountId> = reviews.iter().map(|r| r.student_id()).collect();
        let coach_display_names = self
            .coach_names
            .find_coach_display_names(&coach_ids)
            .await?;
        let student_display_names = self
            .student_names
            .find_student_display_names(&student_ids)
            .await?;
        Ok(LowRatedReviewList {
            reviews,
            lesson_menus,
            coach_display_names,
            student_display_names,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::Mutex,
        time::{Duration, SystemTime},
    };

    use domain::{
        catalog::{LessonMenuId, LessonMenuName, Sport},
        lesson::LessonCompletionId,
        profile::DisplayName,
        review::{
            LessonReview, LessonReviewComment, LessonReviewId, LowRatedReviewHandling, Rating,
            ReviewLessonMenu,
        },
        scheduling::SchedulingError,
    };
    use time::{Date, Month};

    use super::*;
    use crate::{
        Unavailable,
        in_memory::{FixedClock, InMemoryLessonReviewRepository},
    };

    const DAY: Duration = Duration::from_secs(24 * 3600);
    const COACH: AccountId = AccountId::from_u128(3);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    /// レッスン 1 だけがメニュー（ID はレッスンと同じ数）を持つ。
    struct FixedLessonMenus;

    impl ReviewedLessonMenuLookup for FixedLessonMenus {
        async fn find_lesson_menu(
            &self,
            lesson_id: LessonCompletionId,
        ) -> Result<Option<ReviewLessonMenu>, Unavailable> {
            Ok((lesson_id.as_u128() == 1).then(|| menu(1)))
        }
    }

    /// レッスン 1 はテニス、2 は水泳、5 はテニス。他（3・4・6 以降）はスポーツが引けない。問い合わせたレッスンを記録する。
    #[derive(Default)]
    struct FixedLessonSports {
        asked: Mutex<Vec<LessonCompletionId>>,
    }

    impl ReviewedLessonSportLookup for FixedLessonSports {
        async fn find_lesson_sport(
            &self,
            lesson_id: LessonCompletionId,
        ) -> Result<Option<Sport>, Unavailable> {
            self.asked.lock().unwrap().push(lesson_id);
            Ok(match lesson_id.as_u128() {
                1 | 5 => Some(Sport::Tennis),
                2 => Some(Sport::Swimming),
                _ => None,
            })
        }
    }

    fn menu(id: u128) -> ReviewLessonMenu {
        ReviewLessonMenu::new(
            LessonMenuId::from_u128(id),
            LessonMenuName::new(format!("メニュー{id}")).unwrap(),
        )
    }

    /// コーチ 3 だけ表示名が引ける。問い合わせた ID を記録する。
    #[derive(Default)]
    struct FixedNames {
        asked: Mutex<Vec<BTreeSet<AccountId>>>,
    }

    impl FixedNames {
        fn names(&self, ids: &BTreeSet<AccountId>) -> BTreeMap<AccountId, DisplayName> {
            self.asked.lock().unwrap().push(ids.clone());
            ids.iter()
                .filter(|id| **id == COACH || id.as_u128() >= 100)
                .map(|id| {
                    (
                        *id,
                        DisplayName::new(format!("なまえ{}", id.as_u128())).unwrap(),
                    )
                })
                .collect()
        }
    }

    impl ReviewedCoachDisplayNameLookup for FixedNames {
        async fn find_coach_display_names(
            &self,
            coach_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            Ok(self.names(coach_ids))
        }
    }

    impl ReviewerDisplayNameLookup for FixedNames {
        async fn find_student_display_names(
            &self,
            student_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            Ok(self.names(student_ids))
        }
    }

    /// 生徒の ID は 100 + 評価の ID。
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

    type UseCase = ListLowRatedReviews<
        InMemoryLessonReviewRepository,
        FixedLessonMenus,
        FixedLessonSports,
        FixedNames,
        FixedNames,
        FixedClock,
    >;

    fn use_case(reviews: impl IntoIterator<Item = LessonReview>) -> UseCase {
        ListLowRatedReviews::new(
            InMemoryLessonReviewRepository::with_reviews(reviews),
            FixedLessonMenus,
            FixedLessonSports::default(),
            FixedNames::default(),
            FixedNames::default(),
            FixedClock(now()),
        )
    }

    #[tokio::test]
    async fn lists_low_reviews_of_the_last_thirty_days_across_coaches_newest_first() {
        let use_case = use_case([
            review(1, COACH, 1, now() - 3 * DAY),
            review(2, OTHER_COACH, 2, now() - DAY),
            // ★3 は低い評価でない
            review(3, COACH, 3, now() - DAY),
            // 30 日より前は含まない
            review(4, COACH, 1, now() - 30 * DAY - Duration::from_secs(1)),
            // 30 日前ちょうどは含む
            review(5, OTHER_COACH, 2, now() - 30 * DAY),
        ]);

        let list = use_case
            .execute(ListLowRatedReviewsQuery::default())
            .await
            .unwrap();
        let ids: Vec<u128> = list.reviews.iter().map(|r| r.id().as_u128()).collect();
        assert_eq!(ids, [2, 1, 5], "コーチ・スポーツを問わず、新しい順");
        assert!(
            use_case.lesson_sports.asked.lock().unwrap().is_empty(),
            "スポーツを指定しなければスポーツを引かない"
        );
        assert_eq!(
            list.lesson_menus,
            BTreeMap::from([(LessonCompletionId::from_u128(1), menu(1))]),
            "メニューのないレッスンは含まない"
        );
        assert_eq!(
            list.coach_display_names,
            BTreeMap::from([(COACH, DisplayName::new("なまえ3").unwrap())]),
            "引けないコーチ（4）は含まない"
        );
        assert_eq!(
            list.student_display_names
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            [101, 102, 105].map(AccountId::from_u128),
            "載せる評価の生徒だけ"
        );
        assert_eq!(
            use_case.coach_names.asked.lock().unwrap().clone(),
            [BTreeSet::from([COACH, OTHER_COACH])],
            "載せる評価のコーチだけをまとめて 1 回問い合わせる"
        );
    }

    #[tokio::test]
    async fn no_low_reviews_is_an_empty_list_without_asking_names() {
        let use_case = use_case([review(1, COACH, 5, now())]);

        let list = use_case
            .execute(ListLowRatedReviewsQuery::default())
            .await
            .unwrap();
        assert!(list.reviews.is_empty());
        assert!(list.lesson_menus.is_empty());
        assert!(list.coach_display_names.is_empty());
        assert!(list.student_display_names.is_empty());
        assert!(use_case.coach_names.asked.lock().unwrap().is_empty());
        assert!(use_case.student_names.asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_sport_keeps_only_low_reviews_of_lessons_of_that_sport() {
        let use_case = use_case([
            review(1, COACH, 1, now() - 3 * DAY),
            review(2, OTHER_COACH, 2, now() - DAY),
            // ★3 は低い評価でない（スポーツは引かない）
            review(3, COACH, 3, now() - DAY),
            review(5, OTHER_COACH, 2, now() - 30 * DAY),
            // スポーツが引けないレッスンの評価は、どのスポーツにも載らない
            review(6, COACH, 1, now() - 2 * DAY),
        ]);

        let list = use_case
            .execute(ListLowRatedReviewsQuery {
                sport: Some(Sport::Tennis),
                ..Default::default()
            })
            .await
            .unwrap();
        let ids: Vec<u128> = list.reviews.iter().map(|r| r.id().as_u128()).collect();
        assert_eq!(ids, [1, 5], "テニスのレッスンの評価だけを、新しい順");
        assert_eq!(
            use_case.lesson_sports.asked.lock().unwrap().clone(),
            [2, 6, 1, 5].map(LessonCompletionId::from_u128),
            "低い評価のレッスンだけを問い合わせる"
        );
        assert_eq!(
            list.student_display_names
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            [101, 105].map(AccountId::from_u128),
            "絞ったあとの評価の生徒だけ"
        );
        assert_eq!(
            use_case.coach_names.asked.lock().unwrap().clone(),
            [BTreeSet::from([COACH, OTHER_COACH])]
        );

        let list = use_case
            .execute(ListLowRatedReviewsQuery {
                sport: Some(Sport::Golf),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(list.reviews.is_empty(), "そのスポーツの評価がなければ空");
    }

    #[tokio::test]
    async fn handled_keeps_only_reviews_with_or_without_the_mark_of_headquarters() {
        let handled = |review: LessonReview| {
            review.with_handling(Some(LowRatedReviewHandling::new(
                AccountId::from_u128(9),
                now() - DAY,
            )))
        };
        let use_case = use_case([
            handled(review(1, COACH, 1, now() - 3 * DAY)),
            review(2, OTHER_COACH, 2, now() - DAY),
            review(5, OTHER_COACH, 2, now() - 4 * DAY),
            // 30 日より前は印がなくても載らない
            review(4, COACH, 1, now() - 31 * DAY),
        ]);

        let ids = |list: LowRatedReviewList| -> Vec<u128> {
            list.reviews.iter().map(|r| r.id().as_u128()).collect()
        };
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery {
                    handled: Some(false),
                    ..Default::default()
                })
                .await
                .unwrap()),
            [2, 5],
            "未対応だけを、新しい順"
        );
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery {
                    handled: Some(true),
                    ..Default::default()
                })
                .await
                .unwrap()),
            [1],
            "対応済みだけ"
        );
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery::default())
                .await
                .unwrap()),
            [2, 1, 5],
            "指定しなければ印を問わない"
        );
        // スポーツと合わせて絞れる（レッスン 1・5 はテニス、2 は水泳）
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery {
                    sport: Some(Sport::Tennis),
                    handled: Some(false),
                    ..Default::default()
                })
                .await
                .unwrap()),
            [5]
        );
        let list = use_case
            .execute(ListLowRatedReviewsQuery {
                handled: Some(true),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(
            list.student_display_names
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            [AccountId::from_u128(101)],
            "絞ったあとの評価の生徒だけ"
        );
    }

    #[tokio::test]
    async fn coach_id_keeps_only_low_reviews_of_the_coach_combined_with_the_other_filters() {
        let handled = |review: LessonReview| {
            review.with_handling(Some(LowRatedReviewHandling::new(
                AccountId::from_u128(9),
                now() - DAY,
            )))
        };
        let use_case = use_case([
            // レッスン 1・5 はテニス、2 は水泳、6 はスポーツが引けない
            handled(review(1, COACH, 1, now() - 3 * DAY)),
            review(2, COACH, 2, now() - DAY),
            review(5, COACH, 2, now() - 4 * DAY),
            review(6, OTHER_COACH, 1, now() - 2 * DAY),
            // ★3 と 30 日より前は、コーチで絞っても載らない
            review(3, COACH, 3, now() - DAY),
            review(4, COACH, 1, now() - 31 * DAY),
        ]);

        let ids = |list: LowRatedReviewList| -> Vec<u128> {
            list.reviews.iter().map(|r| r.id().as_u128()).collect()
        };
        let list = use_case
            .execute(ListLowRatedReviewsQuery {
                coach_id: Some(COACH),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(
            use_case.coach_names.asked.lock().unwrap().clone(),
            [BTreeSet::from([COACH])],
            "絞ったあとのコーチだけを問い合わせる"
        );
        assert_eq!(ids(list), [2, 1, 5], "そのコーチの評価だけを、新しい順");
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery {
                    coach_id: Some(OTHER_COACH),
                    ..Default::default()
                })
                .await
                .unwrap()),
            [6]
        );
        // 競技・対応済みと合わせて絞れる
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery {
                    sport: Some(Sport::Tennis),
                    coach_id: Some(COACH),
                    ..Default::default()
                })
                .await
                .unwrap()),
            [1, 5]
        );
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery {
                    sport: Some(Sport::Tennis),
                    handled: Some(false),
                    coach_id: Some(COACH),
                    ..Default::default()
                })
                .await
                .unwrap()),
            [5]
        );
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery {
                    handled: Some(true),
                    coach_id: Some(OTHER_COACH),
                    ..Default::default()
                })
                .await
                .unwrap()),
            [] as [u128; 0]
        );
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery {
                    sport: Some(Sport::Tennis),
                    coach_id: Some(OTHER_COACH),
                    ..Default::default()
                })
                .await
                .unwrap()),
            [] as [u128; 0],
            "スポーツが引けないレッスンの評価はコーチが合っても載らない"
        );

        // ない ID・コーチでない ID（生徒）は空の一覧で、表示名を問い合わせない
        let asked = use_case.coach_names.asked.lock().unwrap().len();
        for coach_id in [AccountId::from_u128(99), AccountId::from_u128(101)] {
            let list = use_case
                .execute(ListLowRatedReviewsQuery {
                    coach_id: Some(coach_id),
                    ..Default::default()
                })
                .await
                .unwrap();
            assert!(list.reviews.is_empty(), "{coach_id:?}");
        }
        assert_eq!(use_case.coach_names.asked.lock().unwrap().len(), asked);
    }

    fn date(month: Month, day: u8) -> Date {
        Date::from_calendar_date(2026, month, day).unwrap()
    }

    #[tokio::test]
    async fn from_and_to_keep_only_low_reviews_submitted_in_the_days_in_japan() {
        // 照会した時刻は日本時間の 10 月 31 日 9 時
        let oct24 = calendar_day::start(date(Month::October, 24));
        let oct31 = calendar_day::start(date(Month::October, 31));
        let use_case = use_case([
            // 10 月 24 日の日本時間 0 時ちょうどは含む（レッスン 1 はテニス）
            review(1, COACH, 1, oct24),
            // 10 月 23 日の終わりは含まない
            review(2, COACH, 2, oct24 - Duration::from_nanos(1)),
            // 10 月 30 日の終わりの間際は含む（レッスン 5 はテニス）
            review(5, OTHER_COACH, 2, oct31 - Duration::from_secs(1)),
            // 10 月 31 日（翌日）の日本時間 0 時は含まない
            review(6, COACH, 1, oct31),
            // ★3 は期間の中でも載らない
            review(3, COACH, 3, oct24 + DAY),
        ]);
        let ids = |list: LowRatedReviewList| -> Vec<u128> {
            list.reviews.iter().map(|r| r.id().as_u128()).collect()
        };
        let week = ListLowRatedReviewsQuery {
            from: Some(date(Month::October, 24)),
            to: Some(date(Month::October, 30)),
            ..Default::default()
        };

        assert_eq!(
            ids(use_case.execute(week).await.unwrap()),
            [5, 1],
            "両端の日付を含み、新しい順"
        );
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery { to: None, ..week })
                .await
                .unwrap()),
            [5, 1],
            "from だけなら from から 7 日"
        );
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery { from: None, ..week })
                .await
                .unwrap()),
            [5, 1],
            "to だけなら to までの 7 日"
        );
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery::default())
                .await
                .unwrap()),
            [6, 5, 1, 2],
            "指定しなければ今までどおり"
        );
        // ほかの絞り込みと合わせて絞れる
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery {
                    coach_id: Some(COACH),
                    ..week
                })
                .await
                .unwrap()),
            [1]
        );
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery {
                    sport: Some(Sport::Tennis),
                    handled: Some(false),
                    ..week
                })
                .await
                .unwrap()),
            [5, 1]
        );
        let list = use_case.execute(week).await.unwrap();
        assert_eq!(
            list.student_display_names
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            [101, 105].map(AccountId::from_u128),
            "絞ったあとの評価の生徒だけ"
        );
    }

    #[tokio::test]
    async fn from_and_to_do_not_widen_the_last_thirty_days() {
        // 照会した時刻の 30 日前は 10 月 1 日 0 時（UTC。日本時間の 9 時）
        let since = now() - 30 * DAY;
        let use_case = use_case([
            review(1, COACH, 1, since),
            review(2, COACH, 1, since - Duration::from_secs(1)),
            review(5, COACH, 1, since - 5 * DAY),
        ]);

        let list = use_case
            .execute(ListLowRatedReviewsQuery {
                from: Some(date(Month::September, 20)),
                to: Some(date(Month::October, 1)),
                ..Default::default()
            })
            .await
            .unwrap();
        let ids: Vec<u128> = list.reviews.iter().map(|r| r.id().as_u128()).collect();
        assert_eq!(ids, [1], "30 日より前は期間に入っても載らない");
    }

    #[tokio::test]
    async fn to_before_from_or_more_than_thirty_one_days_is_invalid() {
        let use_case = use_case([review(1, COACH, 1, now() - DAY)]);

        let result = use_case
            .execute(ListLowRatedReviewsQuery {
                from: Some(date(Month::October, 30)),
                to: Some(date(Month::October, 29)),
                ..Default::default()
            })
            .await;
        assert!(
            matches!(
                result,
                Err(ListLowRatedReviewsError::Invalid(
                    SchedulingError::BookingPeriodEndsBeforeItStarts
                ))
            ),
            "{result:?}"
        );

        let result = use_case
            .execute(ListLowRatedReviewsQuery {
                from: Some(date(Month::October, 1)),
                to: Some(date(Month::November, 1)),
                ..Default::default()
            })
            .await;
        assert!(
            matches!(
                result,
                Err(ListLowRatedReviewsError::Invalid(
                    SchedulingError::BookingPeriodTooLong { max_days: 31 }
                ))
            ),
            "{result:?}"
        );
        // 31 日ちょうどは通る
        use_case
            .execute(ListLowRatedReviewsQuery {
                from: Some(date(Month::October, 1)),
                to: Some(date(Month::October, 31)),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(
            use_case.coach_names.asked.lock().unwrap().len() == 1,
            "期間が規則を満たさなければ表示名も引かない"
        );
    }

    #[tokio::test]
    async fn student_id_keeps_only_low_reviews_by_the_student_combined_with_the_other_filters() {
        const STUDENT: AccountId = AccountId::from_u128(200);
        let by_student = |review: LessonReview| {
            LessonReview::restore(
                review.id(),
                review.lesson_id(),
                STUDENT,
                review.coach_id(),
                review.rating(),
                review.comment().clone(),
                review.submitted_at(),
            )
        };
        let handled = |review: LessonReview| {
            review.with_handling(Some(LowRatedReviewHandling::new(
                AccountId::from_u128(9),
                now() - DAY,
            )))
        };
        let use_case = use_case([
            // 生徒 200 の評価。レッスン 1・5 はテニス、2 は水泳
            handled(by_student(review(1, COACH, 1, now() - 3 * DAY))),
            by_student(review(2, OTHER_COACH, 2, now() - DAY)),
            by_student(review(5, COACH, 2, now() - 4 * DAY)),
            // ★3 と 30 日より前は、生徒で絞っても載らない
            by_student(review(3, COACH, 3, now() - DAY)),
            by_student(review(4, COACH, 1, now() - 31 * DAY)),
            // 別の生徒（106）の評価
            review(6, COACH, 1, now() - 2 * DAY),
        ]);

        let ids = |list: LowRatedReviewList| -> Vec<u128> {
            list.reviews.iter().map(|r| r.id().as_u128()).collect()
        };
        let list = use_case
            .execute(ListLowRatedReviewsQuery {
                student_id: Some(STUDENT),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(
            use_case.student_names.asked.lock().unwrap().clone(),
            [BTreeSet::from([STUDENT])],
            "絞ったあとの生徒だけを問い合わせる"
        );
        assert_eq!(ids(list), [2, 1, 5], "その生徒の評価だけを、新しい順");
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery {
                    student_id: Some(AccountId::from_u128(106)),
                    ..Default::default()
                })
                .await
                .unwrap()),
            [6]
        );
        // 競技・対応済み・コーチと合わせて絞れる
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery {
                    sport: Some(Sport::Tennis),
                    student_id: Some(STUDENT),
                    ..Default::default()
                })
                .await
                .unwrap()),
            [1, 5]
        );
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery {
                    handled: Some(false),
                    student_id: Some(STUDENT),
                    ..Default::default()
                })
                .await
                .unwrap()),
            [2, 5]
        );
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery {
                    coach_id: Some(COACH),
                    student_id: Some(STUDENT),
                    ..Default::default()
                })
                .await
                .unwrap()),
            [1, 5]
        );
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery {
                    sport: Some(Sport::Tennis),
                    handled: Some(false),
                    coach_id: Some(COACH),
                    student_id: Some(STUDENT),
                    ..Default::default()
                })
                .await
                .unwrap()),
            [5]
        );
        assert_eq!(
            ids(use_case
                .execute(ListLowRatedReviewsQuery {
                    coach_id: Some(OTHER_COACH),
                    student_id: Some(AccountId::from_u128(106)),
                    ..Default::default()
                })
                .await
                .unwrap()),
            [] as [u128; 0],
            "コーチと生徒の両方に合う評価だけ"
        );

        // ない ID・生徒でない ID（コーチ）は空の一覧で、表示名を問い合わせない
        let asked = use_case.student_names.asked.lock().unwrap().len();
        for student_id in [AccountId::from_u128(99), COACH] {
            let list = use_case
                .execute(ListLowRatedReviewsQuery {
                    student_id: Some(student_id),
                    ..Default::default()
                })
                .await
                .unwrap();
            assert!(list.reviews.is_empty(), "{student_id:?}");
        }
        assert_eq!(use_case.student_names.asked.lock().unwrap().len(), asked);
    }
}
