use std::collections::BTreeSet;

use domain::{
    catalog::Sport, identity::AccountId, lesson::CoachLessonCount,
    scheduling::CoachAcceptanceStatus,
};

use super::{CoachAcceptanceLookup, CoachRecentLessonCountLookup, ListedCoach};
use crate::{Unavailable, identity::AccountRepository};

/// コーチの一覧のユースケース（admin-web の `listCoaches`。本部だけが呼ぶ）。
///
/// コーチのアカウントを表示名の順（Unicode のコードポイントの順。同じ表示名は ID の順）で、1 件ごとに受付の状態
/// （scheduling に問い合わせる。[`CoachAcceptanceLookup`]）と、直近 30 日に終わったレッスンの数（完了記録に問い合わせる。
/// [`CoachRecentLessonCountLookup`]。数え方と期間は domain の [`CoachLessonCount::tally_recent`] で、ここでは数えない）を
/// 添えて返す。受付を止めているコーチも載せる。
/// 任意の条件で絞れる: 受付の状態（`acceptance`）・競技（`sport`。コーチの提供スポーツに含むもの）。両方なら両方を満たすもの、
/// どちらも `None` なら絞らない。並べ方は絞っても変わらない。
/// 誰が呼べるか（本部だけ）は境界（interface）が確かめる。
pub struct ListCoaches<R, A, L> {
    accounts: R,
    acceptances: A,
    recent_lessons: L,
}

impl<R, A, L> ListCoaches<R, A, L>
where
    R: AccountRepository,
    A: CoachAcceptanceLookup,
    L: CoachRecentLessonCountLookup,
{
    pub fn new(accounts: R, acceptances: A, recent_lessons: L) -> Self {
        Self {
            accounts,
            acceptances,
            recent_lessons,
        }
    }

    pub async fn execute(
        &self,
        acceptance: Option<CoachAcceptanceStatus>,
        sport: Option<Sport>,
    ) -> Result<Vec<ListedCoach>, Unavailable> {
        let coaches: Vec<_> = self
            .accounts
            .list_coaches()
            .await?
            .into_iter()
            .filter(|account| {
                sport
                    .is_none_or(|sport| account.as_coach().is_some_and(|coach| coach.offers(sport)))
            })
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
            .filter(|(_, status)| acceptance.is_none_or(|wanted| *status == wanted))
            .collect();
        let coach_ids: BTreeSet<AccountId> =
            coaches.iter().map(|(account, _)| account.id()).collect();
        let mut recent_lessons = if coach_ids.is_empty() {
            Default::default()
        } else {
            self.recent_lessons
                .find_recent_lesson_counts(&coach_ids)
                .await?
        };
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
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::VideoId,
        identity::{Account, Email, Role},
        lesson::{LessonCompletion, LessonCompletionId, LessonSource},
        live_lesson::SessionId,
        profile::DisplayName,
    };

    use super::*;
    use crate::{
        in_memory::{
            FixedClock, InMemoryAccountRepository, InMemoryCoachAcceptanceRepository,
            InMemoryLessonCompletionRepository,
        },
        lesson::LessonCoachRecentLessonCountLookup,
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

        let coaches = ListCoaches::new(accounts, no_suspension(), no_lessons())
            .execute(None, None)
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

        let coaches = ListCoaches::new(accounts, no_suspension(), no_lessons())
            .execute(None, None)
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
        )
        .execute(None, None)
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
        )
        .execute(acceptance, sport)
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
        );

        let listed: Vec<_> = use_case
            .execute(None, None)
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

        let filtered = use_case.execute(None, Some(Sport::Tennis)).await.unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(
            filtered[0].recent_lessons.total_count(),
            3,
            "絞っても数え方は変わらない"
        );
    }
}
