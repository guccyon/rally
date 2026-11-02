use std::{cmp::Reverse, collections::BTreeMap};

use domain::{identity::AccountId, review::Rating};

use super::{CoachReviewList, LessonReviewRepository, ReviewedLessonMenuLookup};
use crate::Unavailable;

/// コーチが受けた評価の一覧のユースケース（admin-web・student-app の `listCoachReviews`。本部と生徒と、自分の分だけの
/// コーチが呼ぶ）。
/// 照会だけで、何も保存しない。
///
/// コーチへの評価を、送った時刻の新しい順に返す。同じ時刻は評価の ID の順。評価がなければ空。
/// 点の上限を指定されたら、点がそれ以下の評価（domain の [`domain::review::LessonReview::is_rated_at_most`]）だけを
/// 残す（並べ方は変えない）。
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

    /// `max_rating` があれば、点がそれ以下の評価だけ。なければ点を問わない。
    pub async fn execute(
        &self,
        coach_id: AccountId,
        max_rating: Option<Rating>,
    ) -> Result<CoachReviewList, Unavailable> {
        let mut reviews = self.reviews.find_by_coach(coach_id).await?;
        if let Some(max_rating) = max_rating {
            reviews.retain(|review| review.is_rated_at_most(max_rating));
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

    use super::*;
    use crate::in_memory::InMemoryLessonReviewRepository;

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

        let list = use_case.execute(COACH, None).await.unwrap();
        let ids: Vec<LessonReviewId> = list.reviews.iter().map(LessonReview::id).collect();
        assert_eq!(
            ids,
            [2, 4, 5, 1].map(LessonReviewId::from_u128),
            "新しい順で、他のコーチの評価を含まない"
        );

        let empty = use_case.execute(NO_REVIEWS_COACH, None).await.unwrap();
        assert!(empty.reviews.is_empty());
        assert!(empty.lesson_menus.is_empty());
    }

    #[tokio::test]
    async fn adds_the_lesson_menu_of_each_reviewed_lesson_that_has_one() {
        let list = use_case().execute(COACH, None).await.unwrap();
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

        let list = use_case
            .execute(COACH, Some(Rating::new(2).unwrap()))
            .await
            .unwrap();
        assert_eq!(
            list.lesson_menus,
            BTreeMap::from([(LessonCompletionId::from_u128(1), menu(1))]),
            "残した評価のメニューだけ"
        );
        assert_eq!(ids(list), [5, 6, 1], "その点以下だけを、新しい順で");
        assert_eq!(
            ids(use_case
                .execute(COACH, Some(Rating::new(3).unwrap()))
                .await
                .unwrap()),
            [4, 5, 6, 1],
            "その点ちょうどを含む"
        );
        assert_eq!(
            ids(use_case
                .execute(COACH, Some(Rating::new(5).unwrap()))
                .await
                .unwrap()),
            [2, 4, 5, 6, 1],
            "5 以下はすべて"
        );
        assert!(
            ids(use_case
                .execute(NO_REVIEWS_COACH, Some(Rating::new(1).unwrap()))
                .await
                .unwrap())
            .is_empty()
        );
    }
}
