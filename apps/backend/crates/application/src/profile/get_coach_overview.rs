use std::collections::BTreeSet;

use domain::{identity::AccountId, scheduling::CoachAcceptanceStatus};

use super::{CoachAcceptanceLookup, CoachOverview};
use crate::{
    Unavailable,
    catalog::{LessonMenuRepository, ListCoachLessonMenus},
    clock::Clock,
    identity::AccountRepository,
    lesson::{GetMyLessonStats, LessonCompletionRepository, MonthlyReviewSummaryLookup},
    review::{GetCoachLowRatedReviewCount, GetCoachReviewSummary, LessonReviewRepository},
};

/// 本部が見るコーチ 1 人の様子のユースケース（admin-web の `getCoachOverview`。本部だけが呼ぶ）。照会だけで、何も保存しない。
///
/// 表示名はアカウントの保存先から読み、受付の状態は scheduling に問い合わせる（[`CoachAcceptanceLookup`]。`ListCoaches` と
/// 同じポート）。評価の件数と平均・直近の低い評価の件数と未対応の件数・今月と先月のレッスンの実績・公開中のレッスンメニューは、
/// それぞれ review などのユースケース（[`GetCoachReviewSummary`]・[`GetCoachLowRatedReviewCount`]・[`GetMyLessonStats`]・
/// [`ListCoachLessonMenus`]）を呼んで、その結果をそのまま使う（数え方・低い評価の線・暦月の区切り・公開中の判定はここで
/// 書かない）。
///
/// `coach_id` がコーチのアカウントでなければ（生徒・本部のアカウントや、ないアカウント）`None`。誰が呼べるか（本部だけ）は
/// 境界（interface）が確かめる。
pub struct GetCoachOverview<R, A, V, M, W, C, N> {
    accounts: R,
    acceptances: A,
    review_summary: GetCoachReviewSummary<V>,
    low_rated_reviews: GetCoachLowRatedReviewCount<V, C>,
    lesson_stats: GetMyLessonStats<M, W, C>,
    lesson_menus: ListCoachLessonMenus<N>,
}

impl<R, A, V, M, W, C, N> GetCoachOverview<R, A, V, M, W, C, N>
where
    R: AccountRepository,
    A: CoachAcceptanceLookup,
    V: LessonReviewRepository,
    M: LessonCompletionRepository,
    W: MonthlyReviewSummaryLookup,
    C: Clock,
    N: LessonMenuRepository,
{
    pub fn new(
        accounts: R,
        acceptances: A,
        review_summary: GetCoachReviewSummary<V>,
        low_rated_reviews: GetCoachLowRatedReviewCount<V, C>,
        lesson_stats: GetMyLessonStats<M, W, C>,
        lesson_menus: ListCoachLessonMenus<N>,
    ) -> Self {
        Self {
            accounts,
            acceptances,
            review_summary,
            low_rated_reviews,
            lesson_stats,
            lesson_menus,
        }
    }

    pub async fn execute(&self, coach_id: AccountId) -> Result<Option<CoachOverview>, Unavailable> {
        let Some(account) = self.accounts.find(coach_id).await? else {
            return Ok(None);
        };
        if account.as_coach().is_none() {
            return Ok(None);
        }
        let acceptance = self
            .acceptances
            .find_coach_acceptances(&BTreeSet::from([coach_id]))
            .await?
            .get(&coach_id)
            .copied()
            .unwrap_or(CoachAcceptanceStatus::Accepting);
        let reviews = self.review_summary.execute(coach_id).await?;
        let low_rated_reviews = self.low_rated_reviews.execute(coach_id).await?;
        let lesson_stats = self.lesson_stats.execute(coach_id).await?;
        let published_lesson_menus = self.lesson_menus.execute(coach_id, None).await?;
        Ok(Some(CoachOverview {
            display_name: account.display_name().clone(),
            acceptance,
            reviews,
            low_rated_reviews,
            lesson_stats,
            published_lesson_menu_count: u32::try_from(published_lesson_menus.len())
                .unwrap_or(u32::MAX),
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::VideoId,
        catalog::{
            LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription, LessonMenuId,
            LessonMenuName, Sport,
        },
        entitlement::YearMonth,
        identity::{Account, Email, Role},
        lesson::{LessonCompletion, LessonCompletionId, LessonSource},
        live_lesson::SessionId,
        profile::DisplayName,
        review::{
            LessonReview, LessonReviewComment, LessonReviewId, LowRatedReviewHandling, Rating,
        },
    };

    use super::*;
    use crate::{
        in_memory::{
            FixedClock, InMemoryAccountRepository, InMemoryCoachAcceptanceRepository,
            InMemoryLessonCompletionRepository, InMemoryLessonMenuRepository,
            InMemoryLessonReviewRepository,
        },
        review::ReviewMonthlyReviewSummaryLookup,
        scheduling::{CoachAcceptanceRepository, SchedulingCoachAcceptanceLookup},
    };

    const COACH: AccountId = AccountId::from_u128(1);
    const OTHER_COACH: AccountId = AccountId::from_u128(2);
    const STUDENT: AccountId = AccountId::from_u128(3);
    const HEADQUARTERS: AccountId = AccountId::from_u128(4);
    const NO_ACCOUNT: AccountId = AccountId::from_u128(99);
    /// 2026-11-01T00:00:00+09:00（2026-10-31T15:00:00Z）
    const NOV1_JST: Duration = Duration::from_secs(1_793_458_800);
    /// 2026-10-01T00:00:00+09:00（2026-09-30T15:00:00Z）
    const OCT1_JST: Duration = Duration::from_secs(1_790_780_400);

    fn nov1() -> SystemTime {
        SystemTime::UNIX_EPOCH + NOV1_JST
    }

    fn oct1() -> SystemTime {
        SystemTime::UNIX_EPOCH + OCT1_JST
    }

    fn account(id: AccountId, display_name: &str, role: Role) -> Account {
        let sports = if role == Role::Headquarters {
            vec![]
        } else {
            vec![Sport::Tennis]
        };
        Account::new(
            id,
            Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
            DisplayName::new(display_name).unwrap(),
            role,
            sports,
            SystemTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    fn completion(id: u128, coach_id: AccountId, live: bool, at: SystemTime) -> LessonCompletion {
        let source = if live {
            LessonSource::Live(SessionId::from_u128(id))
        } else {
            LessonSource::Async(VideoId::from_u128(id))
        };
        LessonCompletion::restore(
            LessonCompletionId::from_u128(id),
            STUDENT,
            coach_id,
            source,
            at,
            false,
        )
    }

    fn review(id: u128, coach_id: AccountId, rating: i64, at: SystemTime) -> LessonReview {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonCompletionId::from_u128(id),
            STUDENT,
            coach_id,
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("よい").unwrap(),
            at,
        )
    }

    fn menu(id: u128, coach_id: AccountId, sport: Sport, is_published: bool) -> LessonMenu {
        LessonMenu::restore(
            LessonMenuId::from_u128(id),
            coach_id,
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

    type UseCase = GetCoachOverview<
        InMemoryAccountRepository,
        SchedulingCoachAcceptanceLookup<InMemoryCoachAcceptanceRepository>,
        InMemoryLessonReviewRepository,
        InMemoryLessonCompletionRepository,
        ReviewMonthlyReviewSummaryLookup<InMemoryLessonReviewRepository>,
        FixedClock,
        InMemoryLessonMenuRepository,
    >;

    /// 今は日本時間の 11 月 1 日 1 時。コーチ `COACH` は受付を止められていて、11 月に通話 1、10 月に通話 1・添削 1、
    /// 9 月に添削 1 を完了し、評価は 5（11 月）・4・2（本部が対応済み）・1（10 月）・2（9 月）を受け、公開中のメニューは
    /// テニス 1・ゴルフ 1（ほかに非公開 1）。`OTHER_COACH` の分は混ぜない。
    async fn use_case() -> UseCase {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(COACH, "たなか", Role::Coach),
            account(OTHER_COACH, "いとう", Role::Coach),
            account(STUDENT, "あおき", Role::Student),
            account(HEADQUARTERS, "うえの", Role::Headquarters),
        ] {
            accounts.insert(account).await.unwrap();
        }
        let acceptances = InMemoryCoachAcceptanceRepository::default();
        acceptances
            .update(COACH, |a| a.suspend(HEADQUARTERS, SystemTime::UNIX_EPOCH))
            .await
            .unwrap();
        let second = Duration::from_secs(1);
        let day = Duration::from_secs(24 * 3600);
        let now = nov1() + Duration::from_secs(3600);
        let reviews = InMemoryLessonReviewRepository::with_reviews([
            review(1, COACH, 5, nov1()),
            review(2, COACH, 4, nov1() - second),
            // 9 月 30 日の★2 は照会した時刻の 30 日より前
            review(3, COACH, 2, oct1() - second),
            review(4, OTHER_COACH, 1, nov1()),
            review(5, COACH, 2, nov1() - day)
                .with_handling(Some(LowRatedReviewHandling::new(HEADQUARTERS, nov1()))),
            review(6, COACH, 1, nov1() - 2 * day),
        ]);
        GetCoachOverview::new(
            accounts,
            SchedulingCoachAcceptanceLookup::new(acceptances),
            GetCoachReviewSummary::new(reviews.clone()),
            GetCoachLowRatedReviewCount::new(reviews.clone(), FixedClock(now)),
            GetMyLessonStats::new(
                InMemoryLessonCompletionRepository::with_completions([
                    completion(1, COACH, true, nov1()),
                    completion(2, COACH, true, nov1() - second),
                    completion(3, COACH, false, oct1()),
                    completion(4, COACH, false, oct1() - second),
                    completion(5, OTHER_COACH, true, nov1()),
                ]),
                ReviewMonthlyReviewSummaryLookup::new(reviews),
                FixedClock(now),
            ),
            ListCoachLessonMenus::new(InMemoryLessonMenuRepository::with_menus([
                menu(1, COACH, Sport::Tennis, true),
                menu(2, COACH, Sport::Golf, true),
                menu(3, COACH, Sport::Tennis, false),
                menu(4, OTHER_COACH, Sport::Tennis, true),
            ])),
        )
    }

    #[tokio::test]
    async fn combines_acceptance_reviews_lesson_stats_and_published_menus_of_the_coach() {
        let overview = use_case().await.execute(COACH).await.unwrap().unwrap();

        assert_eq!(overview.display_name.as_str(), "たなか");
        assert_eq!(overview.acceptance, CoachAcceptanceStatus::Suspended);
        assert_eq!(overview.reviews.coach_id(), COACH);
        assert_eq!(
            (
                overview.reviews.review_count(),
                overview.reviews.average_rating()
            ),
            (5, Some(2.8)),
            "評価は期間で絞らずこれまでのすべて"
        );
        assert_eq!(overview.low_rated_reviews.coach_id(), COACH);
        assert_eq!(
            (
                overview.low_rated_reviews.low_rated_count(),
                overview.low_rated_reviews.unhandled_count()
            ),
            (2, 1),
            "低い評価は直近 30 日の★2 以下で、未対応は対応済みの印のないもの"
        );
        let current = &overview.lesson_stats.current_month;
        let previous = &overview.lesson_stats.previous_month;
        assert_eq!(current.month, YearMonth::new(2026, 11).unwrap());
        assert_eq!(
            (current.lessons.live_count(), current.lessons.async_count()),
            (1, 0)
        );
        assert_eq!(previous.month, YearMonth::new(2026, 10).unwrap());
        assert_eq!(
            (
                previous.lessons.live_count(),
                previous.lessons.async_count()
            ),
            (1, 1),
            "暦月は日本時間で区切り、9 月の分は数えない"
        );
        assert_eq!(
            overview.published_lesson_menu_count, 2,
            "公開中だけをスポーツを問わず数える"
        );
    }

    #[tokio::test]
    async fn a_coach_without_anything_is_accepting_with_zero_counts() {
        let overview = use_case()
            .await
            .execute(OTHER_COACH)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(overview.acceptance, CoachAcceptanceStatus::Accepting);
        assert_eq!(overview.reviews.review_count(), 1);
        assert_eq!(
            (
                overview.low_rated_reviews.low_rated_count(),
                overview.low_rated_reviews.unhandled_count()
            ),
            (1, 1)
        );
        assert_eq!(overview.published_lesson_menu_count, 1);

        let accounts = InMemoryAccountRepository::default();
        accounts
            .insert(account(COACH, "たなか", Role::Coach))
            .await
            .unwrap();
        let reviews = InMemoryLessonReviewRepository::default();
        let empty = GetCoachOverview::new(
            accounts,
            SchedulingCoachAcceptanceLookup::new(InMemoryCoachAcceptanceRepository::default()),
            GetCoachReviewSummary::new(reviews.clone()),
            GetCoachLowRatedReviewCount::new(reviews.clone(), FixedClock(nov1())),
            GetMyLessonStats::new(
                InMemoryLessonCompletionRepository::default(),
                ReviewMonthlyReviewSummaryLookup::new(reviews),
                FixedClock(nov1()),
            ),
            ListCoachLessonMenus::new(InMemoryLessonMenuRepository::default()),
        )
        .execute(COACH)
        .await
        .unwrap()
        .unwrap();
        assert_eq!(empty.acceptance, CoachAcceptanceStatus::Accepting);
        assert_eq!(
            (empty.reviews.review_count(), empty.reviews.average_rating()),
            (0, None)
        );
        assert_eq!(
            (
                empty.low_rated_reviews.low_rated_count(),
                empty.low_rated_reviews.unhandled_count()
            ),
            (0, 0)
        );
        for month in [
            &empty.lesson_stats.current_month,
            &empty.lesson_stats.previous_month,
        ] {
            assert_eq!(
                (month.lessons.live_count(), month.lessons.async_count()),
                (0, 0)
            );
        }
        assert_eq!(empty.published_lesson_menu_count, 0);
    }

    #[tokio::test]
    async fn not_a_coach_is_none() {
        let use_case = use_case().await;
        for id in [STUDENT, HEADQUARTERS, NO_ACCOUNT] {
            assert_eq!(use_case.execute(id).await.unwrap(), None, "{id:?}");
        }
    }
}
