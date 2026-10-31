use std::collections::{BTreeMap, BTreeSet};

use domain::{catalog::Sport, identity::AccountId, review::LowRatedReviewCriteria};

use super::{
    LessonReviewRepository, LowRatedReviewList, ReviewedCoachDisplayNameLookup,
    ReviewedLessonMenuLookup, ReviewedLessonSportLookup, ReviewerDisplayNameLookup,
};
use crate::{Unavailable, clock::Clock};

/// 本部が見る低い評価の一覧のユースケース（admin-web の `listLowRatedLessonReviews`。本部だけが呼ぶ）。
/// 照会だけで、何も保存しない。
///
/// 期間の始まり（domain の [`LowRatedReviewCriteria::since`]）以降に送られた評価を読み、低い評価を domain の
/// [`LowRatedReviewCriteria::select`] で選んで並べる（しきい値と期間は domain だけ）。スポーツを指定されたら、評価した
/// 完了したレッスンのスポーツ（[`ReviewedLessonSportLookup`]。レッスンの元の枠・動画のスポーツ）がそれと同じ評価だけを残す
/// （スポーツが引けない評価は残さない）。各評価には、評価した完了した
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

    /// `sport` があれば、そのスポーツのレッスンの評価だけ。なければスポーツを問わない。
    pub async fn execute(&self, sport: Option<Sport>) -> Result<LowRatedReviewList, Unavailable> {
        let now = self.clock.now();
        let submitted = self
            .reviews
            .find_submitted_since(LowRatedReviewCriteria::since(now))
            .await?;
        let mut reviews = LowRatedReviewCriteria::select(submitted, now);
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
        review::{LessonReview, LessonReviewComment, LessonReviewId, Rating, ReviewLessonMenu},
    };

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryLessonReviewRepository};

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

        let list = use_case.execute(None).await.unwrap();
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

        let list = use_case.execute(None).await.unwrap();
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

        let list = use_case.execute(Some(Sport::Tennis)).await.unwrap();
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

        let list = use_case.execute(Some(Sport::Golf)).await.unwrap();
        assert!(list.reviews.is_empty(), "そのスポーツの評価がなければ空");
    }
}
