use std::collections::BTreeSet;

use domain::{
    identity::AccountId,
    lesson::{CoachLessonCount, InactiveCoachFilter},
    messaging::UnansweredCoachFilter,
    profile::{CoachAcceptanceFilter, CoachSportFilter},
    review::UnhandledLowRatedCoachFilter,
    scheduling::CoachAcceptanceStatus,
};

use super::{
    CoachAcceptanceLookup, CoachLowRatedReviewCountLookup, CoachRecentLessonCountLookup,
    CoachUnansweredMessageThreadCountsLookup, ListCoachesQuery, ListedCoach,
};
use crate::{Unavailable, identity::AccountRepository};

/// コーチの一覧のユースケース（admin-web の `listCoaches`。本部だけが呼ぶ）。
///
/// コーチのアカウントを表示名の順（Unicode のコードポイントの順。同じ表示名は ID の順）で、1 件ごとに受付の状態
/// （scheduling に問い合わせる。[`CoachAcceptanceLookup`]）と、直近 30 日に終わったレッスンの数（完了記録に問い合わせる。
/// [`CoachRecentLessonCountLookup`]。数え方と期間は domain の [`CoachLessonCount::tally_recent`] で、ここでは数えない）を
/// 添えて返す。受付を止めているコーチも載せる。
/// 任意の条件（[`ListCoachesQuery`]）で絞れる: 表示名の検索語（`name`。部分一致の規則は domain の
/// [`domain::profile::DisplayNameQuery`] で、ここでは比べ方を持たない）・受付の状態（`acceptance`。判定は domain の
/// [`CoachAcceptanceFilter`]）・競技（`sport`。コーチの
/// 提供スポーツに含むもの。判定は domain の [`CoachSportFilter`]）・直近にレッスンのないコーチ（`inactive`。添える直近の
/// レッスンの数が 0 のもの。判定は domain の [`InactiveCoachFilter`]）・未対応の低い評価のあるコーチ
/// （`has_unhandled_low_rated`。review に問い合わせる。[`CoachLowRatedReviewCountLookup`]。数え方は domain の
/// [`domain::review::CoachLowRatedReviewCount::tally`] で、判定は [`UnhandledLowRatedCoachFilter`]。絞るときだけ問い合わせる）・
/// 返事のないメッセージのあるコーチ（`has_unanswered`。messaging に問い合わせる。
/// [`CoachUnansweredMessageThreadCountsLookup`]。数え方は `getCoachOverview` と同じ domain の
/// [`domain::messaging::CoachUnansweredMessageThreadCount::tally`] で、判定は [`UnansweredCoachFilter`]。絞るときだけ
/// 問い合わせる）。
/// 付けたものをすべて満たすもの、どれも付けなければ絞らない。
/// 並べ方は絞っても変わらない。
/// 誰が呼べるか（本部だけ）は境界（interface）が確かめる。
pub struct ListCoaches<R, A, L, N, U> {
    accounts: R,
    acceptances: A,
    recent_lessons: L,
    low_rated_reviews: N,
    unanswered_threads: U,
}

impl<R, A, L, N, U> ListCoaches<R, A, L, N, U>
where
    R: AccountRepository,
    A: CoachAcceptanceLookup,
    L: CoachRecentLessonCountLookup,
    N: CoachLowRatedReviewCountLookup,
    U: CoachUnansweredMessageThreadCountsLookup,
{
    pub fn new(
        accounts: R,
        acceptances: A,
        recent_lessons: L,
        low_rated_reviews: N,
        unanswered_threads: U,
    ) -> Self {
        Self {
            accounts,
            acceptances,
            recent_lessons,
            low_rated_reviews,
            unanswered_threads,
        }
    }

    pub async fn execute(&self, query: &ListCoachesQuery) -> Result<Vec<ListedCoach>, Unavailable> {
        let ListCoachesQuery {
            name,
            acceptance,
            sport,
            inactive,
            has_unhandled_low_rated,
            has_unanswered,
        } = query;
        let coaches: Vec<_> = self
            .accounts
            .list_coaches()
            .await?
            .into_iter()
            .filter(|account| {
                name.as_ref()
                    .is_none_or(|name| name.matches(account.display_name()))
            })
            .filter(|account| CoachSportFilter::new(*sport).matches(account))
            .collect();
        let coach_ids: BTreeSet<AccountId> = coaches.iter().map(|account| account.id()).collect();
        let acceptances = if coach_ids.is_empty() {
            Default::default()
        } else {
            self.acceptances.find_coach_acceptances(&coach_ids).await?
        };
        let coaches: Vec<_> = coaches
            .into_iter()
            .map(|account| {
                let status = acceptances
                    .get(&account.id())
                    .copied()
                    .unwrap_or(CoachAcceptanceStatus::Accepting);
                (account, status)
            })
            .filter(|(_, status)| CoachAcceptanceFilter::new(*acceptance).matches(*status))
            .collect();
        let unhandled_low_rated = UnhandledLowRatedCoachFilter::new(*has_unhandled_low_rated);
        // 絞らないときは評価に問い合わせない（今までどおり）
        let coaches: Vec<_> =
            if unhandled_low_rated.has_unhandled_low_rated() && !coaches.is_empty() {
                let coach_ids: BTreeSet<AccountId> =
                    coaches.iter().map(|(account, _)| account.id()).collect();
                let low_rated_reviews = self
                    .low_rated_reviews
                    .find_low_rated_review_counts(&coach_ids)
                    .await?;
                coaches
                    .into_iter()
                    // ポートは聞いたコーチをすべて返す。返さなかったコーチは 0 件として扱う（載せない）
                    .filter(|(account, _)| {
                        low_rated_reviews
                            .get(&account.id())
                            .is_some_and(|count| unhandled_low_rated.matches(count))
                    })
                    .collect()
            } else {
                coaches
            };
        let unanswered = UnansweredCoachFilter::new(*has_unanswered);
        // 絞らないときは messaging に問い合わせない（今までどおり）
        let coaches: Vec<_> = if unanswered.has_unanswered() && !coaches.is_empty() {
            let coach_ids: BTreeSet<AccountId> =
                coaches.iter().map(|(account, _)| account.id()).collect();
            let counts = self
                .unanswered_threads
                .find_unanswered_message_thread_counts(&coach_ids)
                .await?;
            coaches
                .into_iter()
                // ポートは聞いたコーチをすべて返す。返さなかったコーチは 0 件として扱う（載せない）
                .filter(|(account, _)| {
                    counts
                        .get(&account.id())
                        .is_some_and(|count| unanswered.matches(count))
                })
                .collect()
        } else {
            coaches
        };
        let coach_ids: BTreeSet<AccountId> =
            coaches.iter().map(|(account, _)| account.id()).collect();
        let mut recent_lessons = if coach_ids.is_empty() {
            Default::default()
        } else {
            self.recent_lessons
                .find_recent_lesson_counts(&coach_ids)
                .await?
        };
        let inactive = InactiveCoachFilter::new(*inactive);
        Ok(coaches
            .into_iter()
            .map(|(account, acceptance)| ListedCoach {
                // ポートは聞いたコーチをすべて返す。返さなかったコーチは 0 件として扱う
                recent_lessons: recent_lessons
                    .remove(&account.id())
                    .unwrap_or_else(|| CoachLessonCount::tally(account.id(), &[])),
                acceptance,
                account,
            })
            .filter(|coach| inactive.matches(&coach.recent_lessons))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::VideoId,
        catalog::Sport,
        identity::{Account, Email, Role},
        lesson::{LessonCompletion, LessonCompletionId, LessonSource},
        live_lesson::SessionId,
        messaging::{
            CoachUnansweredMessageThreadCount, MessageBody, MessageContent, MessageId,
            MessageThread, MessageThreadId,
        },
        profile::{DisplayName, DisplayNameQuery},
        review::{
            CoachLowRatedReviewCount, LessonReview, LessonReviewComment, LessonReviewId,
            LowRatedReviewHandling, Rating,
        },
    };

    use super::*;
    use crate::{
        in_memory::{
            FixedClock, InMemoryAccountRepository, InMemoryCoachAcceptanceRepository,
            InMemoryLessonCompletionRepository, InMemoryLessonReviewRepository,
            InMemoryMessageThreadRepository,
        },
        lesson::LessonCoachRecentLessonCountLookup,
        messaging::MessagingCoachUnansweredMessageThreadCountLookup,
        review::ReviewCoachLowRatedReviewCountLookup,
        scheduling::{CoachAcceptanceRepository, SchedulingCoachAcceptanceLookup},
    };

    /// 受付を止める・再開する本部のアカウント。
    const HEADQUARTERS_ID: AccountId = AccountId::from_u128(900);

    type Acceptances = SchedulingCoachAcceptanceLookup<InMemoryCoachAcceptanceRepository>;

    fn no_suspension() -> Acceptances {
        SchedulingCoachAcceptanceLookup::new(InMemoryCoachAcceptanceRepository::default())
    }

    type RecentLessons =
        LessonCoachRecentLessonCountLookup<InMemoryLessonCompletionRepository, FixedClock>;

    const DAY: Duration = Duration::from_secs(24 * 3600);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    fn no_lessons() -> RecentLessons {
        recent_lessons([])
    }

    fn recent_lessons(completions: impl IntoIterator<Item = LessonCompletion>) -> RecentLessons {
        LessonCoachRecentLessonCountLookup::new(
            InMemoryLessonCompletionRepository::with_completions(completions),
            FixedClock(now()),
        )
    }

    type LowRatedReviews =
        ReviewCoachLowRatedReviewCountLookup<InMemoryLessonReviewRepository, FixedClock>;

    fn no_reviews() -> LowRatedReviews {
        low_rated_reviews([])
    }

    fn low_rated_reviews(reviews: impl IntoIterator<Item = LessonReview>) -> LowRatedReviews {
        ReviewCoachLowRatedReviewCountLookup::new(
            InMemoryLessonReviewRepository::with_reviews(reviews),
            FixedClock(now()),
        )
    }

    fn review(id: u128, coach_id: u128, rating: i64, at: SystemTime) -> LessonReview {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonCompletionId::from_u128(id),
            AccountId::from_u128(100 + id),
            AccountId::from_u128(coach_id),
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("いまひとつ").unwrap(),
            at,
        )
    }

    fn handled(review: LessonReview) -> LessonReview {
        review.with_handling(Some(LowRatedReviewHandling::new(HEADQUARTERS_ID, now())))
    }

    /// 呼ばれたら失敗する評価の問い合わせ（絞らないときは問い合わせないことを確かめる）。
    struct UnreachableLowRatedReviews;

    impl CoachLowRatedReviewCountLookup for UnreachableLowRatedReviews {
        async fn find_low_rated_review_counts(
            &self,
            _coach_ids: &BTreeSet<AccountId>,
        ) -> Result<std::collections::BTreeMap<AccountId, CoachLowRatedReviewCount>, Unavailable>
        {
            panic!("絞らないときは評価に問い合わせない")
        }
    }

    type UnansweredThreads = MessagingCoachUnansweredMessageThreadCountLookup<
        InMemoryMessageThreadRepository,
        FixedClock,
    >;

    fn no_threads() -> UnansweredThreads {
        unanswered_threads([])
    }

    fn unanswered_threads(threads: impl IntoIterator<Item = MessageThread>) -> UnansweredThreads {
        MessagingCoachUnansweredMessageThreadCountLookup::new(
            InMemoryMessageThreadRepository::with_threads(threads),
            FixedClock(now()),
        )
    }

    const HOUR: Duration = Duration::from_secs(3600);

    /// `id` の生徒（`100 + id`）とコーチ `coach_id` のスレッドを照会の 10 日前に開き、`exchange` の順に
    /// （生徒が送ったか, 送信時刻）で本文を送る。
    fn message_thread(id: u128, coach_id: u128, exchange: &[(bool, SystemTime)]) -> MessageThread {
        let student = AccountId::from_u128(100 + id);
        let coach = AccountId::from_u128(coach_id);
        let mut thread = MessageThread::open(
            MessageThreadId::from_u128(id),
            student,
            coach,
            now() - 10 * DAY,
        )
        .unwrap();
        for (n, (by_student, at)) in exchange.iter().enumerate() {
            thread
                .send(
                    MessageId::from_u128(id * 100 + n as u128),
                    if *by_student { student } else { coach },
                    MessageContent::Text(MessageBody::new("質問です").unwrap()),
                    true,
                    *at,
                )
                .unwrap();
        }
        thread
    }

    /// 呼ばれたら失敗するスレッドの問い合わせ（絞らないときは問い合わせないことを確かめる）。
    struct UnreachableUnansweredThreads;

    impl CoachUnansweredMessageThreadCountsLookup for UnreachableUnansweredThreads {
        async fn find_unanswered_message_thread_counts(
            &self,
            _coach_ids: &BTreeSet<AccountId>,
        ) -> Result<
            std::collections::BTreeMap<AccountId, CoachUnansweredMessageThreadCount>,
            Unavailable,
        > {
            panic!("絞らないときはスレッドに問い合わせない")
        }
    }

    fn completion(id: u128, coach_id: u128, live: bool, at: SystemTime) -> LessonCompletion {
        let source = if live {
            LessonSource::Live(SessionId::from_u128(id))
        } else {
            LessonSource::Async(VideoId::from_u128(id))
        };
        LessonCompletion::restore(
            LessonCompletionId::from_u128(id),
            AccountId::from_u128(100 + id),
            AccountId::from_u128(coach_id),
            source,
            at,
            false,
        )
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

    #[tokio::test]
    async fn lists_only_coaches_in_display_name_order() {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, "たなか", Role::Coach, &[Sport::Tennis]),
            account(2, "あおき", Role::Student, &[Sport::Tennis]),
            account(3, "いとう", Role::Coach, &[Sport::Soccer, Sport::Golf]),
            account(4, "うえの", Role::Headquarters, &[]),
            // 同じ表示名は ID の順
            account(6, "いとう", Role::Coach, &[Sport::Baseball]),
            account(5, "いとう", Role::Coach, &[Sport::Basketball]),
        ] {
            accounts.insert(account).await.unwrap();
        }

        let coaches = ListCoaches::new(
            accounts,
            no_suspension(),
            no_lessons(),
            no_reviews(),
            no_threads(),
        )
        .execute(&ListCoachesQuery::default())
        .await
        .unwrap();
        let listed: Vec<_> = coaches
            .iter()
            .map(|c| (c.account.id().as_u128(), c.account.display_name().as_str()))
            .collect();
        assert_eq!(
            listed,
            [(3, "いとう"), (5, "いとう"), (6, "いとう"), (1, "たなか")]
        );
    }

    #[tokio::test]
    async fn no_coaches_is_an_empty_list() {
        let accounts = InMemoryAccountRepository::default();
        accounts
            .insert(account(1, "あおき", Role::Student, &[Sport::Tennis]))
            .await
            .unwrap();

        let coaches = ListCoaches::new(
            accounts,
            no_suspension(),
            no_lessons(),
            no_reviews(),
            no_threads(),
        )
        .execute(&ListCoachesQuery::default())
        .await
        .unwrap();
        assert!(coaches.is_empty());
    }

    #[tokio::test]
    async fn each_coach_has_its_acceptance_status_and_suspended_coaches_are_listed() {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, "たなか", Role::Coach, &[Sport::Tennis]),
            account(2, "いとう", Role::Coach, &[Sport::Soccer]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        let acceptances = InMemoryCoachAcceptanceRepository::default();
        acceptances
            .update(AccountId::from_u128(1), |a| {
                a.suspend(
                    HEADQUARTERS_ID,
                    domain::scheduling::CoachSuspensionReason::default(),
                    SystemTime::UNIX_EPOCH,
                )
            })
            .await
            .unwrap();

        let coaches = ListCoaches::new(
            accounts,
            SchedulingCoachAcceptanceLookup::new(acceptances),
            no_lessons(),
            no_reviews(),
            no_threads(),
        )
        .execute(&ListCoachesQuery::default())
        .await
        .unwrap();
        let listed: Vec<_> = coaches
            .iter()
            .map(|c| (c.account.id().as_u128(), c.acceptance))
            .collect();
        assert_eq!(
            listed,
            [
                (2, CoachAcceptanceStatus::Accepting),
                (1, CoachAcceptanceStatus::Suspended)
            ]
        );
    }

    /// たなか（1。テニス・停止中）・いとう（2。サッカーとテニス）・うえの（3。野球・停止中）と、テニスの生徒（4）。
    async fn filtered(
        acceptance: Option<CoachAcceptanceStatus>,
        sport: Option<Sport>,
    ) -> Vec<(u128, CoachAcceptanceStatus)> {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, "たなか", Role::Coach, &[Sport::Tennis]),
            account(2, "いとう", Role::Coach, &[Sport::Soccer, Sport::Tennis]),
            account(3, "うえの", Role::Coach, &[Sport::Baseball]),
            account(4, "あおき", Role::Student, &[Sport::Tennis]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        let acceptances = InMemoryCoachAcceptanceRepository::default();
        for id in [1, 3] {
            acceptances
                .update(AccountId::from_u128(id), |a| {
                    a.suspend(
                        HEADQUARTERS_ID,
                        domain::scheduling::CoachSuspensionReason::default(),
                        SystemTime::UNIX_EPOCH,
                    )
                })
                .await
                .unwrap();
        }
        ListCoaches::new(
            accounts,
            SchedulingCoachAcceptanceLookup::new(acceptances),
            no_lessons(),
            no_reviews(),
            no_threads(),
        )
        .execute(&ListCoachesQuery {
            acceptance,
            sport,
            ..ListCoachesQuery::default()
        })
        .await
        .unwrap()
        .iter()
        .map(|c| (c.account.id().as_u128(), c.acceptance))
        .collect()
    }

    #[tokio::test]
    async fn filters_by_acceptance_status_keeping_the_order() {
        use CoachAcceptanceStatus::{Accepting, Suspended};
        assert_eq!(
            filtered(Some(Suspended), None).await,
            [(3, Suspended), (1, Suspended)]
        );
        assert_eq!(filtered(Some(Accepting), None).await, [(2, Accepting)]);
    }

    #[tokio::test]
    async fn filters_by_sport_the_coach_offers() {
        use CoachAcceptanceStatus::{Accepting, Suspended};
        assert_eq!(
            filtered(None, Some(Sport::Tennis)).await,
            [(2, Accepting), (1, Suspended)],
            "提供スポーツのどれかが一致すれば載る。テニスの生徒は載らない"
        );
        assert!(filtered(None, Some(Sport::Golf)).await.is_empty());
    }

    #[tokio::test]
    async fn both_conditions_must_hold_and_no_condition_lists_all() {
        use CoachAcceptanceStatus::{Accepting, Suspended};
        assert_eq!(
            filtered(Some(Suspended), Some(Sport::Tennis)).await,
            [(1, Suspended)]
        );
        assert!(
            filtered(Some(Accepting), Some(Sport::Baseball))
                .await
                .is_empty()
        );
        assert_eq!(
            filtered(None, None).await,
            [(2, Accepting), (3, Suspended), (1, Suspended)]
        );
    }

    #[tokio::test]
    async fn each_coach_has_the_number_of_lessons_completed_in_the_last_30_days() {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, "たなか", Role::Coach, &[Sport::Tennis]),
            account(2, "いとう", Role::Coach, &[Sport::Soccer]),
            account(3, "うえの", Role::Coach, &[Sport::Baseball]),
            account(4, "あおき", Role::Student, &[Sport::Tennis]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        let use_case = ListCoaches::new(
            accounts,
            no_suspension(),
            recent_lessons([
                completion(1, 1, true, now()),
                completion(2, 1, false, now() - DAY),
                // 30 日前ちょうどは含み、それより前は数えない
                completion(3, 1, false, now() - 30 * DAY),
                completion(4, 1, true, now() - 30 * DAY - Duration::from_secs(1)),
                completion(5, 2, false, now() - 2 * DAY),
            ]),
            no_reviews(),
            no_threads(),
        );

        let listed: Vec<_> = use_case
            .execute(&ListCoachesQuery::default())
            .await
            .unwrap()
            .iter()
            .map(|c| {
                (
                    c.account.id().as_u128(),
                    c.recent_lessons.coach_id().as_u128(),
                    c.recent_lessons.live_count(),
                    c.recent_lessons.async_count(),
                )
            })
            .collect();
        assert_eq!(
            listed,
            [(2, 2, 0, 1), (3, 3, 0, 0), (1, 1, 1, 2)],
            "同期・非同期を数え、レッスンのないコーチは 0 件"
        );

        let filtered = use_case
            .execute(&ListCoachesQuery {
                sport: Some(Sport::Tennis),
                ..ListCoachesQuery::default()
            })
            .await
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(
            filtered[0].recent_lessons.total_count(),
            3,
            "絞っても数え方は変わらない"
        );
    }

    /// 直近 30 日にレッスンのないコーチに絞った一覧（ID の並び）。たなか（1。テニス・停止中。30 日前ちょうどに添削 1）・
    /// いとう（2。サッカーとテニス。31 日前に通話 1 だけ）・うえの（3。テニス。レッスンなし）・えんどう（4。野球。今日通話 1）と、
    /// レッスンのないテニスの生徒（5）。
    async fn inactive(query: ListCoachesQuery) -> Vec<u128> {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, "たなか", Role::Coach, &[Sport::Tennis]),
            account(2, "いとう", Role::Coach, &[Sport::Soccer, Sport::Tennis]),
            account(3, "うえの", Role::Coach, &[Sport::Tennis]),
            account(4, "えんどう", Role::Coach, &[Sport::Baseball]),
            account(5, "あおき", Role::Student, &[Sport::Tennis]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        let acceptances = InMemoryCoachAcceptanceRepository::default();
        acceptances
            .update(AccountId::from_u128(1), |a| {
                a.suspend(
                    HEADQUARTERS_ID,
                    domain::scheduling::CoachSuspensionReason::default(),
                    SystemTime::UNIX_EPOCH,
                )
            })
            .await
            .unwrap();
        ListCoaches::new(
            accounts,
            SchedulingCoachAcceptanceLookup::new(acceptances),
            recent_lessons([
                completion(1, 1, false, now() - 30 * DAY),
                completion(2, 2, true, now() - 31 * DAY),
                completion(3, 4, true, now()),
            ]),
            no_reviews(),
            no_threads(),
        )
        .execute(&query)
        .await
        .unwrap()
        .iter()
        .map(|c| {
            if query.inactive {
                assert_eq!(c.recent_lessons.total_count(), 0, "{c:?}");
            }
            c.account.id().as_u128()
        })
        .collect()
    }

    fn inactive_only() -> ListCoachesQuery {
        ListCoachesQuery {
            inactive: true,
            ..ListCoachesQuery::default()
        }
    }

    #[tokio::test]
    async fn inactive_lists_only_coaches_without_lessons_in_the_last_30_days_keeping_the_order() {
        // 表示名の順（いとう・うえの）のまま。30 日前ちょうどのレッスンは数えるのでたなかは載らない。生徒は載らない
        assert_eq!(inactive(inactive_only()).await, [2, 3]);
        // 付けなければ（false なら）今までどおりすべて
        assert_eq!(inactive(ListCoachesQuery::default()).await, [2, 3, 4, 1]);
    }

    #[tokio::test]
    async fn inactive_is_combined_with_the_other_conditions() {
        use CoachAcceptanceStatus::{Accepting, Suspended};
        assert_eq!(
            inactive(ListCoachesQuery {
                sport: Some(Sport::Soccer),
                ..inactive_only()
            })
            .await,
            [2]
        );
        assert!(
            inactive(ListCoachesQuery {
                acceptance: Some(Suspended),
                ..inactive_only()
            })
            .await
            .is_empty(),
            "停止中のたなかにはレッスンがある"
        );
        assert_eq!(
            inactive(ListCoachesQuery {
                acceptance: Some(Accepting),
                sport: Some(Sport::Tennis),
                name: DisplayNameQuery::new("うえ").unwrap(),
                ..inactive_only()
            })
            .await,
            [3]
        );
        assert!(
            inactive(ListCoachesQuery {
                sport: Some(Sport::Baseball),
                ..inactive_only()
            })
            .await
            .is_empty()
        );
    }

    /// 未対応の低い評価のあるコーチに絞った一覧（ID の並び）。たなか（1。テニス・停止中。今日 ★1 が未対応）・
    /// いとう（2。サッカーとテニス。★2 は対応済みで、31 日前の ★1 は期間の外）・うえの（3。テニス。30 日前ちょうどの ★2 が
    /// 未対応。今日通話 1）・えんどう（4。野球。★3 だけ）と、テニスの生徒（5）。
    async fn unhandled_low_rated(query: ListCoachesQuery) -> Vec<u128> {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, "たなか", Role::Coach, &[Sport::Tennis]),
            account(2, "いとう", Role::Coach, &[Sport::Soccer, Sport::Tennis]),
            account(3, "うえの", Role::Coach, &[Sport::Tennis]),
            account(4, "えんどう", Role::Coach, &[Sport::Baseball]),
            account(5, "あおき", Role::Student, &[Sport::Tennis]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        let acceptances = InMemoryCoachAcceptanceRepository::default();
        acceptances
            .update(AccountId::from_u128(1), |a| {
                a.suspend(
                    HEADQUARTERS_ID,
                    domain::scheduling::CoachSuspensionReason::default(),
                    SystemTime::UNIX_EPOCH,
                )
            })
            .await
            .unwrap();
        ListCoaches::new(
            accounts,
            SchedulingCoachAcceptanceLookup::new(acceptances),
            recent_lessons([completion(1, 3, true, now())]),
            low_rated_reviews([
                review(1, 1, 1, now()),
                handled(review(2, 2, 2, now() - DAY)),
                review(3, 2, 1, now() - 31 * DAY),
                review(4, 3, 2, now() - 30 * DAY),
                review(5, 4, 3, now()),
                review(6, 5, 1, now()),
            ]),
            no_threads(),
        )
        .execute(&query)
        .await
        .unwrap()
        .iter()
        .map(|c| c.account.id().as_u128())
        .collect()
    }

    fn unhandled_low_rated_only() -> ListCoachesQuery {
        ListCoachesQuery {
            has_unhandled_low_rated: true,
            ..ListCoachesQuery::default()
        }
    }

    #[tokio::test]
    async fn has_unhandled_low_rated_lists_only_coaches_with_unhandled_low_reviews_keeping_the_order()
     {
        // 表示名の順（うえの・たなか）のまま。対応済み・期間の外・★3 だけのコーチと生徒は載らない
        assert_eq!(
            unhandled_low_rated(unhandled_low_rated_only()).await,
            [3, 1]
        );
        // 付けなければ（false なら）今までどおりすべて
        assert_eq!(
            unhandled_low_rated(ListCoachesQuery::default()).await,
            [2, 3, 4, 1]
        );
    }

    #[tokio::test]
    async fn has_unhandled_low_rated_is_combined_with_the_other_conditions() {
        use CoachAcceptanceStatus::{Accepting, Suspended};
        let query = |query: ListCoachesQuery| ListCoachesQuery {
            has_unhandled_low_rated: true,
            ..query
        };
        assert_eq!(
            unhandled_low_rated(query(ListCoachesQuery {
                acceptance: Some(Suspended),
                ..ListCoachesQuery::default()
            }))
            .await,
            [1]
        );
        assert_eq!(
            unhandled_low_rated(query(ListCoachesQuery {
                acceptance: Some(Accepting),
                sport: Some(Sport::Tennis),
                ..ListCoachesQuery::default()
            }))
            .await,
            [3]
        );
        assert!(
            unhandled_low_rated(query(ListCoachesQuery {
                sport: Some(Sport::Soccer),
                ..ListCoachesQuery::default()
            }))
            .await
            .is_empty(),
            "いとうの低い評価は対応済みか期間の外"
        );
        assert_eq!(
            unhandled_low_rated(query(ListCoachesQuery {
                name: DisplayNameQuery::new("たな").unwrap(),
                ..ListCoachesQuery::default()
            }))
            .await,
            [1]
        );
        assert_eq!(
            unhandled_low_rated(query(inactive_only())).await,
            [1],
            "うえのには直近のレッスンがある"
        );
    }

    #[tokio::test]
    async fn without_has_unhandled_low_rated_reviews_are_not_looked_up() {
        let accounts = InMemoryAccountRepository::default();
        accounts
            .insert(account(1, "たなか", Role::Coach, &[Sport::Tennis]))
            .await
            .unwrap();
        let coaches = ListCoaches::new(
            accounts,
            no_suspension(),
            no_lessons(),
            UnreachableLowRatedReviews,
            no_threads(),
        )
        .execute(&ListCoachesQuery {
            inactive: true,
            sport: Some(Sport::Tennis),
            ..ListCoachesQuery::default()
        })
        .await
        .unwrap();
        assert_eq!(coaches.len(), 1);
    }

    /// 返事のないメッセージのあるコーチに絞った一覧（ID の並び）。たなか（1。テニス・停止中。24 時間ちょうど前の生徒の発言に
    /// 返信がない）・いとう（2。サッカーとテニス。返信済みと、24 時間に 1 秒足りないものだけ）・うえの（3。テニス。30 時間前の
    /// 発言に返信がなく、未対応の ★1 がある。今日通話 1）・えんどう（4。野球。スレッドなし）と、テニスの生徒（5）。
    async fn unanswered(query: ListCoachesQuery) -> Vec<u128> {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, "たなか", Role::Coach, &[Sport::Tennis]),
            account(2, "いとう", Role::Coach, &[Sport::Soccer, Sport::Tennis]),
            account(3, "うえの", Role::Coach, &[Sport::Tennis]),
            account(4, "えんどう", Role::Coach, &[Sport::Baseball]),
            account(5, "あおき", Role::Student, &[Sport::Tennis]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        let acceptances = InMemoryCoachAcceptanceRepository::default();
        acceptances
            .update(AccountId::from_u128(1), |a| {
                a.suspend(
                    HEADQUARTERS_ID,
                    domain::scheduling::CoachSuspensionReason::default(),
                    SystemTime::UNIX_EPOCH,
                )
            })
            .await
            .unwrap();
        ListCoaches::new(
            accounts,
            SchedulingCoachAcceptanceLookup::new(acceptances),
            recent_lessons([completion(1, 3, true, now())]),
            low_rated_reviews([review(1, 3, 1, now())]),
            unanswered_threads([
                message_thread(1, 1, &[(true, now() - 24 * HOUR)]),
                message_thread(2, 2, &[(true, now() - 50 * HOUR), (false, now() - HOUR)]),
                message_thread(3, 2, &[(true, now() - 24 * HOUR + Duration::from_secs(1))]),
                message_thread(4, 3, &[(true, now() - 30 * HOUR)]),
                // 生徒の ID をコーチとしたスレッドはない（生徒は載らない）
            ]),
        )
        .execute(&query)
        .await
        .unwrap()
        .iter()
        .map(|c| c.account.id().as_u128())
        .collect()
    }

    fn unanswered_only() -> ListCoachesQuery {
        ListCoachesQuery {
            has_unanswered: true,
            ..ListCoachesQuery::default()
        }
    }

    #[tokio::test]
    async fn has_unanswered_lists_only_coaches_with_an_unanswered_thread_keeping_the_order() {
        // 表示名の順（うえの・たなか）のまま。返信済み・時間に満たない・スレッドのないコーチと生徒は載らない
        assert_eq!(unanswered(unanswered_only()).await, [3, 1]);
        // 付けなければ（false なら）今までどおりすべて
        assert_eq!(unanswered(ListCoachesQuery::default()).await, [2, 3, 4, 1]);
    }

    #[tokio::test]
    async fn has_unanswered_is_combined_with_the_other_conditions() {
        use CoachAcceptanceStatus::{Accepting, Suspended};
        let query = |query: ListCoachesQuery| ListCoachesQuery {
            has_unanswered: true,
            ..query
        };
        assert_eq!(
            unanswered(query(ListCoachesQuery {
                acceptance: Some(Suspended),
                ..ListCoachesQuery::default()
            }))
            .await,
            [1]
        );
        assert_eq!(
            unanswered(query(ListCoachesQuery {
                acceptance: Some(Accepting),
                sport: Some(Sport::Tennis),
                ..ListCoachesQuery::default()
            }))
            .await,
            [3]
        );
        assert!(
            unanswered(query(ListCoachesQuery {
                sport: Some(Sport::Soccer),
                ..ListCoachesQuery::default()
            }))
            .await
            .is_empty(),
            "いとうのスレッドは返信済みか時間に満たない"
        );
        assert_eq!(
            unanswered(query(ListCoachesQuery {
                name: DisplayNameQuery::new("たな").unwrap(),
                ..ListCoachesQuery::default()
            }))
            .await,
            [1]
        );
        assert_eq!(
            unanswered(query(inactive_only())).await,
            [1],
            "うえのには直近のレッスンがある"
        );
        assert_eq!(
            unanswered(query(unhandled_low_rated_only())).await,
            [3],
            "未対応の低い評価があるのはうえのだけ"
        );
    }

    #[tokio::test]
    async fn without_has_unanswered_threads_are_not_looked_up() {
        let accounts = InMemoryAccountRepository::default();
        accounts
            .insert(account(1, "たなか", Role::Coach, &[Sport::Tennis]))
            .await
            .unwrap();
        let coaches = ListCoaches::new(
            accounts,
            no_suspension(),
            no_lessons(),
            no_reviews(),
            UnreachableUnansweredThreads,
        )
        .execute(&ListCoachesQuery {
            inactive: true,
            sport: Some(Sport::Tennis),
            ..ListCoachesQuery::default()
        })
        .await
        .unwrap();
        assert_eq!(coaches.len(), 1);
    }

    /// 表示名で探すコーチの一覧（ID の並び）。Taro Yamada（1。テニス・停止中）・山田 花子（2。サッカー）・
    /// やまだ（3。テニス）・山田（4。テニス）と、表示名の一致する生徒（5）。
    async fn by_name(query: ListCoachesQuery) -> Vec<u128> {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, "Taro Yamada", Role::Coach, &[Sport::Tennis]),
            account(2, "山田 花子", Role::Coach, &[Sport::Soccer]),
            account(3, "やまだ", Role::Coach, &[Sport::Tennis]),
            account(4, "山田", Role::Coach, &[Sport::Tennis]),
            account(5, "山田 生徒", Role::Student, &[Sport::Tennis]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        let acceptances = InMemoryCoachAcceptanceRepository::default();
        acceptances
            .update(AccountId::from_u128(1), |a| {
                a.suspend(
                    HEADQUARTERS_ID,
                    domain::scheduling::CoachSuspensionReason::default(),
                    SystemTime::UNIX_EPOCH,
                )
            })
            .await
            .unwrap();
        ListCoaches::new(
            accounts,
            SchedulingCoachAcceptanceLookup::new(acceptances),
            no_lessons(),
            no_reviews(),
            no_threads(),
        )
        .execute(&query)
        .await
        .unwrap()
        .iter()
        .map(|c| c.account.id().as_u128())
        .collect()
    }

    fn name(value: &str) -> ListCoachesQuery {
        ListCoachesQuery {
            name: DisplayNameQuery::new(value).unwrap(),
            ..ListCoachesQuery::default()
        }
    }

    #[tokio::test]
    async fn name_lists_only_coaches_whose_display_name_contains_it_keeping_the_order() {
        // 表示名のコードポイントの順（Taro Yamada・やまだ・山田・山田 花子）のまま。生徒は表示名が一致しても載らない
        assert_eq!(by_name(name("山田")).await, [4, 2]);
        // ASCII の英字の大文字・小文字は区別せず、ひらがなと漢字は同じとみなさない
        assert_eq!(by_name(name("YAMADA")).await, [1]);
        assert_eq!(by_name(name("やまだ")).await, [3]);
        // 途中の空白はそのまま比べる
        assert_eq!(by_name(name(" 山田 ")).await, [4, 2]);
        assert_eq!(by_name(name("山田 花")).await, [2]);
        assert!(by_name(name("いない")).await.is_empty());
        // 検索語がなければ絞らない
        assert_eq!(by_name(name("　")).await, [1, 3, 4, 2]);
    }

    #[tokio::test]
    async fn name_is_combined_with_acceptance_status_and_sport() {
        use CoachAcceptanceStatus::{Accepting, Suspended};
        let query = |value, acceptance, sport| ListCoachesQuery {
            acceptance,
            sport,
            ..name(value)
        };
        assert_eq!(by_name(query("a", None, Some(Sport::Tennis))).await, [1]);
        assert!(
            by_name(query("a", Some(Accepting), None)).await.is_empty(),
            "Taro Yamada は停止中"
        );
        assert_eq!(by_name(query("a", Some(Suspended), None)).await, [1]);
        assert_eq!(
            by_name(query("山田", Some(Accepting), Some(Sport::Tennis))).await,
            [4]
        );
        assert!(
            by_name(query("山田", None, Some(Sport::Golf)))
                .await
                .is_empty()
        );
    }
}
