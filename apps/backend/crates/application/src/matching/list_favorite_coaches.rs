use std::collections::BTreeSet;

use domain::{identity::AccountId, review::CoachReviewSummary};

use super::{CoachReviewSummaryLookup, FavoriteCoachListing, FavoriteCoachRepository};
use crate::{Unavailable, identity::AccountRepository};

/// お気に入りのコーチの一覧のユースケース（student-app の `listFavoriteCoaches`。生徒が呼ぶ）。
/// 照会だけで、何も保存しない。行動イベントも境界間イベントも発火しない。
///
/// 生徒のお気に入りを、入れた時刻の新しい順（同じ時刻はコーチの ID の順）に、コーチの表示名と提供スポーツと
/// 評価の件数と平均を添えて返す。コーチのアカウントがなくなった・コーチでなくなったお気に入りは載せない。どの
/// スポーツをどのクライアントに見せるか（student-app はゴルフを扱わない）は境界が決める。
pub struct ListFavoriteCoaches<F, R, V> {
    favorites: F,
    accounts: R,
    reviews: V,
}

impl<F, R, V> ListFavoriteCoaches<F, R, V>
where
    F: FavoriteCoachRepository,
    R: AccountRepository,
    V: CoachReviewSummaryLookup,
{
    pub fn new(favorites: F, accounts: R, reviews: V) -> Self {
        Self {
            favorites,
            accounts,
            reviews,
        }
    }

    /// 生徒 `student_id`（認証したアカウント。役割の確認は境界が行う）のお気に入りのコーチ。
    pub async fn execute(
        &self,
        student_id: AccountId,
    ) -> Result<Vec<FavoriteCoachListing>, Unavailable> {
        let favorites = self.favorites.find_by_student(student_id).await?;
        let mut coaches = Vec::with_capacity(favorites.len());
        for favorite in favorites {
            let Some(account) = self.accounts.find(favorite.coach_id()).await? else {
                continue;
            };
            if let Some(coach) = account.as_coach() {
                coaches.push((
                    favorite,
                    account.display_name().clone(),
                    coach.sports().clone(),
                ));
            }
        }
        let coach_ids: BTreeSet<AccountId> = coaches.iter().map(|(f, _, _)| f.coach_id()).collect();
        let mut summaries = self.reviews.find_review_summaries(&coach_ids).await?;
        Ok(coaches
            .into_iter()
            .map(|(favorite, display_name, sports)| FavoriteCoachListing {
                coach_id: favorite.coach_id(),
                display_name,
                sports,
                review_summary: summaries
                    .remove(&favorite.coach_id())
                    .unwrap_or_else(|| CoachReviewSummary::tally(favorite.coach_id(), &[])),
                favorited_at: favorite.favorited_at(),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::Sport,
        identity::{Account, Email, Role},
        lesson::LessonCompletionId,
        matching::FavoriteCoach,
        profile::DisplayName,
        review::{LessonReview, LessonReviewComment, LessonReviewId, Rating},
    };

    use super::*;
    use crate::{
        in_memory::{
            InMemoryAccountRepository, InMemoryFavoriteCoachRepository,
            InMemoryLessonReviewRepository,
        },
        review::ReviewCoachReviewSummaryLookup,
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

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
            OTHER_STUDENT,
            AccountId::from_u128(coach_id),
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("よい").unwrap(),
            SystemTime::UNIX_EPOCH,
        )
    }

    /// 一覧の 1 件の（コーチの ID, 表示名, 提供スポーツ, 評価の件数, 平均, 入れた時刻）。
    type Row<'a> = (u128, &'a str, Vec<Sport>, u32, Option<f64>, SystemTime);

    fn favorite(student_id: AccountId, coach_id: u128, secs: u64) -> FavoriteCoach {
        FavoriteCoach::restore(student_id, AccountId::from_u128(coach_id), at(secs))
    }

    #[tokio::test]
    async fn lists_favorites_newest_first_with_name_sports_and_reviews() {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, "せいと", Role::Student, &[Sport::Tennis]),
            account(2, "せいと2", Role::Student, &[Sport::Tennis]),
            account(3, "たなか", Role::Coach, &[Sport::Tennis, Sport::Golf]),
            account(4, "いとう", Role::Coach, &[Sport::Soccer]),
            account(5, "うえの", Role::Coach, &[Sport::Swimming]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        let favorites = InMemoryFavoriteCoachRepository::with_favorites([
            favorite(STUDENT, 3, 10),
            favorite(STUDENT, 5, 30),
            favorite(STUDENT, 4, 30),
            // コーチのアカウントがない（載せない）
            favorite(STUDENT, 99, 40),
            favorite(OTHER_STUDENT, 3, 50),
        ]);
        let reviews = InMemoryLessonReviewRepository::with_reviews([
            review(1, 3, 5),
            review(2, 3, 4),
            review(3, 4, 2),
        ]);
        let use_case = ListFavoriteCoaches::new(
            favorites,
            accounts,
            ReviewCoachReviewSummaryLookup::new(reviews),
        );

        let listings = use_case.execute(STUDENT).await.unwrap();
        let rows: Vec<Row> = listings
            .iter()
            .map(|l| {
                (
                    l.coach_id.as_u128(),
                    l.display_name.as_str(),
                    l.sports.iter().copied().collect(),
                    l.review_summary.review_count(),
                    l.review_summary.average_rating(),
                    l.favorited_at,
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                // 同じ時刻はコーチの ID の順
                (4, "いとう", vec![Sport::Soccer], 1, Some(2.0), at(30)),
                (5, "うえの", vec![Sport::Swimming], 0, None, at(30)),
                (
                    3,
                    "たなか",
                    vec![Sport::Tennis, Sport::Golf],
                    2,
                    Some(4.5),
                    at(10)
                ),
            ]
        );

        assert!(
            use_case
                .execute(AccountId::from_u128(77))
                .await
                .unwrap()
                .is_empty(),
            "お気に入りがなければ空"
        );
    }
}
