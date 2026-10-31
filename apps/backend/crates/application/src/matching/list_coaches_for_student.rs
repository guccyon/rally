use std::{cmp::Ordering, collections::BTreeSet};

use domain::{catalog::Sport, identity::AccountId, review::CoachReviewSummary};

use super::{
    CoachCandidateLookup, CoachListing, CoachReviewSummaryLookup, PublishedLessonMenuCountLookup,
    StudentSportLookup,
};
use crate::Unavailable;

/// コーチを探す一覧の入力。`student_id` は認証したアカウント（役割の確認は境界が行う）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListCoachesForStudentCommand {
    pub student_id: AccountId,
    /// 省いたら生徒自身のスポーツ
    pub sport: Option<Sport>,
}

#[derive(Debug)]
pub enum ListCoachesForStudentError {
    /// `sport` を省いたのに、`student_id` の生徒のアカウントがない
    StudentNotFound,
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<Unavailable> for ListCoachesForStudentError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}

/// 生徒がスポーツでコーチを探す一覧のユースケース（student-app の `listCoachesForStudent`。生徒が呼ぶ）。
/// 照会だけで、何も保存しない。行動イベントも境界間イベントも発火しない。
///
/// スポーツ（省いたら生徒自身のスポーツ）を提供するコーチをすべて、評価の件数と平均と、そのスポーツの公開中の
/// レッスンメニューの数を添えて返す。並べ方は評価の平均の高い順（評価のないコーチは最後）、同じなら表示名の
/// Unicode のコードポイントの順、それも同じなら ID の順。どのスポーツをどのクライアントに見せるか（student-app は
/// ゴルフを扱わない）は境界が決める。
pub struct ListCoachesForStudent<S, C, R, M> {
    students: S,
    coaches: C,
    reviews: R,
    menus: M,
}

impl<S, C, R, M> ListCoachesForStudent<S, C, R, M>
where
    S: StudentSportLookup,
    C: CoachCandidateLookup,
    R: CoachReviewSummaryLookup,
    M: PublishedLessonMenuCountLookup,
{
    pub fn new(students: S, coaches: C, reviews: R, menus: M) -> Self {
        Self {
            students,
            coaches,
            reviews,
            menus,
        }
    }

    pub async fn execute(
        &self,
        command: ListCoachesForStudentCommand,
    ) -> Result<Vec<CoachListing>, ListCoachesForStudentError> {
        let sport = match command.sport {
            Some(sport) => sport,
            None => self
                .students
                .find_student_sport(command.student_id)
                .await?
                .ok_or(ListCoachesForStudentError::StudentNotFound)?,
        };
        let candidates = self.coaches.find_coaches_offering(sport).await?;
        let coach_ids: BTreeSet<AccountId> = candidates.iter().map(|c| c.coach_id).collect();
        let mut summaries = self.reviews.find_review_summaries(&coach_ids).await?;
        let menu_counts = self
            .menus
            .count_published_lesson_menus(&coach_ids, sport)
            .await?;

        let mut listings: Vec<CoachListing> = candidates
            .into_iter()
            .map(|candidate| CoachListing {
                review_summary: summaries
                    .remove(&candidate.coach_id)
                    .unwrap_or_else(|| CoachReviewSummary::tally(candidate.coach_id, &[])),
                published_lesson_menu_count: menu_counts
                    .get(&candidate.coach_id)
                    .copied()
                    .unwrap_or(0),
                coach_id: candidate.coach_id,
                display_name: candidate.display_name,
                sport,
            })
            .collect();
        listings.sort_by(listing_order);
        Ok(listings)
    }
}

/// 評価の平均の高い順（評価のないコーチは最後）→ 表示名の順 → ID の順。
fn listing_order(a: &CoachListing, b: &CoachListing) -> Ordering {
    let by_average = match (
        a.review_summary.average_rating(),
        b.review_summary.average_rating(),
    ) {
        (Some(a), Some(b)) => b.total_cmp(&a),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    };
    by_average
        .then_with(|| a.display_name.as_str().cmp(b.display_name.as_str()))
        .then_with(|| a.coach_id.cmp(&b.coach_id))
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::{
        catalog::{
            LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription, LessonMenuId,
            LessonMenuName,
        },
        identity::{Account, Email, Role},
        lesson::LessonCompletionId,
        profile::DisplayName,
        review::{LessonReview, LessonReviewComment, LessonReviewId, Rating},
    };

    use super::*;
    use crate::{
        catalog::CatalogPublishedLessonMenuCountLookup,
        identity::AccountRepository,
        in_memory::{
            InMemoryAccountRepository, InMemoryLessonMenuRepository, InMemoryLessonReviewRepository,
        },
        profile::{ProfileCoachCandidateLookup, ProfileStudentSportLookup},
        review::ReviewCoachReviewSummaryLookup,
    };

    const STUDENT: AccountId = AccountId::from_u128(1);

    fn account(id: u128, display_name: &str, role: Role, sports: &[Sport]) -> Account {
        Account::new(
            AccountId::from_u128(id),
            Email::new(format!("user{id}@example.com")).unwrap(),
            DisplayName::new(display_name).unwrap(),
            role,
            sports.iter().copied(),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    fn review(id: u128, coach_id: u128, rating: i64) -> LessonReview {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonCompletionId::from_u128(id),
            STUDENT,
            AccountId::from_u128(coach_id),
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("よい").unwrap(),
            SystemTime::UNIX_EPOCH,
        )
    }

    fn menu(id: u128, coach_id: u128, sport: Sport, is_published: bool) -> LessonMenu {
        LessonMenu::restore(
            LessonMenuId::from_u128(id),
            AccountId::from_u128(coach_id),
            LessonMenuContent {
                sport,
                format: LessonFormat::Async,
                name: LessonMenuName::new("添削").unwrap(),
                description: LessonMenuDescription::default(),
            },
            is_published,
            SystemTime::UNIX_EPOCH,
        )
    }

    type UseCase = ListCoachesForStudent<
        ProfileStudentSportLookup<InMemoryAccountRepository>,
        ProfileCoachCandidateLookup<InMemoryAccountRepository>,
        ReviewCoachReviewSummaryLookup<InMemoryLessonReviewRepository>,
        CatalogPublishedLessonMenuCountLookup<InMemoryLessonMenuRepository>,
    >;

    /// テニスの生徒 1 人と、コーチ 6 人（テニス 5 人・サッカー 1 人）。
    async fn use_case() -> UseCase {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, "せいと", Role::Student, &[Sport::Tennis]),
            account(2, "たなか", Role::Coach, &[Sport::Tennis]),
            account(3, "いとう", Role::Coach, &[Sport::Tennis, Sport::Soccer]),
            account(4, "うえの", Role::Coach, &[Sport::Soccer]),
            account(5, "あおき", Role::Coach, &[Sport::Tennis]),
            // 評価の平均が同じ（4.5）なら表示名の順、表示名も同じなら ID の順
            account(7, "えもと", Role::Coach, &[Sport::Tennis]),
            account(6, "えもと", Role::Coach, &[Sport::Tennis]),
            account(8, "ほんぶ", Role::Headquarters, &[]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        let reviews = InMemoryLessonReviewRepository::with_reviews([
            review(1, 2, 5),
            review(2, 2, 4),
            review(3, 3, 5),
            review(4, 4, 5),
            review(5, 6, 5),
            review(6, 6, 4),
            review(7, 7, 4),
            review(8, 7, 5),
        ]);
        let menus = InMemoryLessonMenuRepository::with_menus([
            menu(1, 2, Sport::Tennis, true),
            menu(2, 2, Sport::Tennis, true),
            menu(3, 2, Sport::Tennis, false),
            menu(4, 3, Sport::Soccer, true),
            menu(5, 3, Sport::Tennis, true),
            menu(6, 4, Sport::Soccer, true),
        ]);
        ListCoachesForStudent::new(
            ProfileStudentSportLookup::new(accounts.clone()),
            ProfileCoachCandidateLookup::new(accounts),
            ReviewCoachReviewSummaryLookup::new(reviews),
            CatalogPublishedLessonMenuCountLookup::new(menus),
        )
    }

    fn command(sport: Option<Sport>) -> ListCoachesForStudentCommand {
        ListCoachesForStudentCommand {
            student_id: STUDENT,
            sport,
        }
    }

    fn rows(listings: &[CoachListing]) -> Vec<(u128, Option<f64>, u32, u32)> {
        listings
            .iter()
            .map(|l| {
                (
                    l.coach_id.as_u128(),
                    l.review_summary.average_rating(),
                    l.review_summary.review_count(),
                    l.published_lesson_menu_count,
                )
            })
            .collect()
    }

    #[tokio::test]
    async fn lists_coaches_of_the_students_sport_by_average_rating_then_display_name() {
        let listings = use_case().await.execute(command(None)).await.unwrap();
        assert!(listings.iter().all(|l| l.sport == Sport::Tennis));
        assert_eq!(
            rows(&listings),
            [
                (3, Some(5.0), 1, 1),
                (6, Some(4.5), 2, 0),
                (7, Some(4.5), 2, 0),
                (2, Some(4.5), 2, 2),
                // 評価のないコーチは最後
                (5, None, 0, 0),
            ],
            "サッカーだけのコーチ（4）と本部（8）と生徒（1）は載せない。メニューの数はテニスの公開中のものだけ"
        );
        assert_eq!(listings[1].display_name.as_str(), "えもと");
        assert_eq!(listings[3].display_name.as_str(), "たなか");
    }

    #[tokio::test]
    async fn sport_filters_and_counts_menus_of_that_sport() {
        let listings = use_case()
            .await
            .execute(command(Some(Sport::Soccer)))
            .await
            .unwrap();
        assert!(listings.iter().all(|l| l.sport == Sport::Soccer));
        assert_eq!(
            rows(&listings),
            [(3, Some(5.0), 1, 1), (4, Some(5.0), 1, 1)],
            "評価は件数と平均をスポーツで分けない。同じ平均なら表示名の順（いとう → うえの）"
        );

        let none = use_case()
            .await
            .execute(command(Some(Sport::Swimming)))
            .await
            .unwrap();
        assert!(none.is_empty());
    }

    #[tokio::test]
    async fn omitted_sport_for_an_unknown_student_is_student_not_found() {
        let use_case = use_case().await;
        for id in [99, 2] {
            let result = use_case
                .execute(ListCoachesForStudentCommand {
                    student_id: AccountId::from_u128(id),
                    sport: None,
                })
                .await;
            assert!(
                matches!(result, Err(ListCoachesForStudentError::StudentNotFound)),
                "{id}: {result:?}"
            );
        }
        // スポーツを付ければ生徒のアカウントは読まない
        let listings = use_case
            .execute(ListCoachesForStudentCommand {
                student_id: AccountId::from_u128(99),
                sport: Some(Sport::Tennis),
            })
            .await
            .unwrap();
        assert_eq!(listings.len(), 5);
    }
}
