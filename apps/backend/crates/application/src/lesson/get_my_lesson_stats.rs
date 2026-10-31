use domain::{
    entitlement::YearMonth,
    identity::AccountId,
    lesson::{CoachLessonCount, LessonCompletion},
    review::CoachReviewSummary,
};

use super::{
    CoachLessonStats, LessonCompletionRepository, MonthlyLessonStats, MonthlyReviewSummaryLookup,
};
use crate::{Unavailable, clock::Clock, entitlement::calendar_month};

/// コーチの今月と先月のレッスンの実績のユースケース（admin-web の `getMyLessonStats`。コーチだけが自分の分を呼ぶ）。
///
/// 暦月は日本時間で区切る（`entitlement::calendar_month` だけを通す）。完了したレッスンは完了記録の保存先から先月の
/// 始まり以後の分を読み、完了した時刻の暦月ごとに domain の [`CoachLessonCount::tally`] で数える。評価の件数と平均は
/// review に [`MonthlyReviewSummaryLookup`] で問い合わせる（評価の表は読まない）。読むだけで何も変えず、
/// 金額（支払い額・手数料）は扱わない。誰が呼べるか（コーチだけ）は境界（interface）が確かめる。
pub struct GetMyLessonStats<M, R, C> {
    completions: M,
    reviews: R,
    clock: C,
}

impl<M, R, C> GetMyLessonStats<M, R, C>
where
    M: LessonCompletionRepository,
    R: MonthlyReviewSummaryLookup,
    C: Clock,
{
    pub fn new(completions: M, reviews: R, clock: C) -> Self {
        Self {
            completions,
            reviews,
            clock,
        }
    }

    pub async fn execute(&self, coach_id: AccountId) -> Result<CoachLessonStats, Unavailable> {
        let current = calendar_month::of(self.clock.now());
        let previous = current.previous();
        let completions = self
            .completions
            .find_by_coach(coach_id, calendar_month::start(previous))
            .await?;
        let mut reviews = self
            .reviews
            .find_monthly_review_summaries(coach_id, &[previous, current])
            .await?;
        let mut stats_of = |month: YearMonth| {
            let in_month: Vec<LessonCompletion> = completions
                .iter()
                .filter(|completion| calendar_month::of(completion.completed_at()) == month)
                .cloned()
                .collect();
            MonthlyLessonStats {
                month,
                lessons: CoachLessonCount::tally(coach_id, &in_month),
                // ポートは聞いた月をすべて返す。返さなかった月は評価が 0 件として扱う
                reviews: reviews
                    .remove(&month)
                    .unwrap_or_else(|| CoachReviewSummary::tally(coach_id, &[])),
            }
        };
        Ok(CoachLessonStats {
            current_month: stats_of(current),
            previous_month: stats_of(previous),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::VideoId,
        lesson::{LessonCompletionId, LessonSource},
        live_lesson::SessionId,
        review::{LessonReview, LessonReviewComment, LessonReviewId, Rating},
    };

    use super::*;
    use crate::{
        in_memory::{
            FixedClock, InMemoryLessonCompletionRepository, InMemoryLessonReviewRepository,
        },
        review::ReviewMonthlyReviewSummaryLookup,
    };

    const COACH: AccountId = AccountId::from_u128(3);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);
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

    fn completion(id: u128, coach_id: AccountId, live: bool, at: SystemTime) -> LessonCompletion {
        let source = if live {
            LessonSource::Live(SessionId::from_u128(id))
        } else {
            LessonSource::Async(VideoId::from_u128(id))
        };
        LessonCompletion::restore(
            LessonCompletionId::from_u128(id),
            AccountId::from_u128(100 + id),
            coach_id,
            source,
            at,
            false,
        )
    }

    fn review(id: u128, rating: i64, at: SystemTime) -> LessonReview {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonCompletionId::from_u128(id),
            AccountId::from_u128(100 + id),
            COACH,
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("よい").unwrap(),
            at,
        )
    }

    fn use_case(
        completions: impl IntoIterator<Item = LessonCompletion>,
        reviews: impl IntoIterator<Item = LessonReview>,
        now: SystemTime,
    ) -> GetMyLessonStats<
        InMemoryLessonCompletionRepository,
        ReviewMonthlyReviewSummaryLookup<InMemoryLessonReviewRepository>,
        FixedClock,
    > {
        GetMyLessonStats::new(
            InMemoryLessonCompletionRepository::with_completions(completions),
            ReviewMonthlyReviewSummaryLookup::new(InMemoryLessonReviewRepository::with_reviews(
                reviews,
            )),
            FixedClock(now),
        )
    }

    fn row(stats: &MonthlyLessonStats) -> (u8, u32, u32, u32, Option<f64>) {
        (
            stats.month.month(),
            stats.lessons.live_count(),
            stats.lessons.async_count(),
            stats.reviews.review_count(),
            stats.reviews.average_rating(),
        )
    }

    #[tokio::test]
    async fn splits_this_month_and_last_month_at_midnight_in_japan() {
        let second = Duration::from_secs(1);
        let stats = use_case(
            [
                // 11 月（日本時間の 1 日 0 時ちょうどから）
                completion(1, COACH, true, nov1()),
                completion(2, COACH, false, nov1() + second),
                // 10 月（日本時間の 10 月 31 日 23:59:59 と、10 月 1 日 0 時ちょうど）
                completion(3, COACH, true, nov1() - second),
                completion(4, COACH, true, oct1()),
                completion(5, COACH, false, oct1() + second),
                // 9 月は先月より前なので数えない
                completion(6, COACH, true, oct1() - second),
                // 他のコーチの分は数えない
                completion(7, OTHER_COACH, true, nov1()),
            ],
            [
                review(1, 5, nov1()),
                review(3, 4, nov1() - second),
                review(4, 3, oct1()),
                review(6, 1, oct1() - second),
            ],
            nov1() + Duration::from_secs(3600),
        )
        .execute(COACH)
        .await
        .unwrap();

        assert_eq!(row(&stats.current_month), (11, 1, 1, 1, Some(5.0)));
        assert_eq!(row(&stats.previous_month), (10, 2, 1, 2, Some(3.5)));
    }

    #[tokio::test]
    async fn just_before_midnight_in_japan_is_still_the_old_month() {
        // 日本時間の 10 月 31 日 23:59:59（UTC では 10 月 31 日 14:59:59）は今月が 10 月、先月が 9 月
        let stats = use_case(
            [completion(1, COACH, true, nov1())],
            [],
            nov1() - Duration::from_secs(1),
        )
        .execute(COACH)
        .await
        .unwrap();
        assert_eq!(stats.current_month.month, YearMonth::new(2026, 10).unwrap());
        assert_eq!(stats.previous_month.month, YearMonth::new(2026, 9).unwrap());
        assert_eq!(row(&stats.current_month), (10, 0, 0, 0, None));
    }

    #[tokio::test]
    async fn a_coach_without_lessons_has_zero_in_both_months() {
        let stats = use_case([], [], nov1()).execute(COACH).await.unwrap();
        assert_eq!(row(&stats.current_month), (11, 0, 0, 0, None));
        assert_eq!(row(&stats.previous_month), (10, 0, 0, 0, None));
        assert_eq!(stats.current_month.lessons.coach_id(), COACH);
    }

    #[tokio::test]
    async fn january_follows_december_of_the_last_year() {
        // 2027-01-01T00:00:00+09:00
        let jan1 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_798_729_200);
        let stats = use_case(
            [completion(1, COACH, false, jan1 - Duration::from_secs(1))],
            [],
            jan1,
        )
        .execute(COACH)
        .await
        .unwrap();
        assert_eq!(stats.current_month.month, YearMonth::new(2027, 1).unwrap());
        assert_eq!(row(&stats.previous_month), (12, 0, 1, 0, None));
    }
}
