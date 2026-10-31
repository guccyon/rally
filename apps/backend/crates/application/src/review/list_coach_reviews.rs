use std::{cmp::Reverse, collections::BTreeMap};

use domain::identity::AccountId;

use super::{CoachReviewList, LessonReviewRepository, ReviewedLessonMenuLookup};
use crate::Unavailable;

/// コーチが受けた評価の一覧のユースケース（admin-web・student-app の `listCoachReviews`。本部と生徒と、自分の分だけの
/// コーチが呼ぶ）。
/// 照会だけで、何も保存しない。
///
/// コーチへの評価を、送った時刻の新しい順に返す。同じ時刻は評価の ID の順。評価がなければ空。
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

    pub async fn execute(&self, coach_id: AccountId) -> Result<CoachReviewList, Unavailable> {
        let mut reviews = self.reviews.find_by_coach(coach_id).await?;
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
        lesson::LessonId,
        review::{LessonReview, LessonReviewComment, LessonReviewId, Rating, ReviewLessonMenu},
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
            lesson_id: LessonId,
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
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonId::from_u128(id),
            AccountId::from_u128(100 + id),
            coach_id,
            Rating::new(4).unwrap(),
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

        let list = use_case.execute(COACH).await.unwrap();
        let ids: Vec<LessonReviewId> = list.reviews.iter().map(LessonReview::id).collect();
        assert_eq!(
            ids,
            [2, 4, 5, 1].map(LessonReviewId::from_u128),
            "新しい順で、他のコーチの評価を含まない"
        );

        let empty = use_case.execute(NO_REVIEWS_COACH).await.unwrap();
        assert!(empty.reviews.is_empty());
        assert!(empty.lesson_menus.is_empty());
    }

    #[tokio::test]
    async fn adds_the_lesson_menu_of_each_reviewed_lesson_that_has_one() {
        let list = use_case().execute(COACH).await.unwrap();
        // メニューのないレッスン（4・5）は含まない
        assert_eq!(
            list.lesson_menus,
            BTreeMap::from([
                (LessonId::from_u128(1), menu(1)),
                (LessonId::from_u128(2), menu(2)),
            ])
        );
    }
}
