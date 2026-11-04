use domain::{identity::AccountId, scheduling::CoachAcceptanceChange};

use super::{
    CoachAcceptanceRepository, CoachIntroductionLookup, ListCoachAcceptanceChangesError,
    ListCoachAcceptanceChangesQuery, calendar_day, suspend_coach::coach_exists,
};

/// コーチ 1 人の受付を止めた・再開した記録の照会のユースケース（本部だけが呼ぶ。admin-web の
/// `listCoachAcceptanceChanges`）。誰が呼べるか（本部だけ）は境界（interface）が確かめる。照会だけで、何も保存しない。
///
/// コーチがいるかを profile に問い合わせ（[`CoachIntroductionLookup`]。停止・再開と同じ）、いれば保存先が返す記録
/// （新しい順。[`CoachAcceptanceRepository::changes`]）をそのまま返す。コーチでないアカウントとないアカウントは `None`。
/// 一度も止めていないコーチは空の一覧。
///
/// 条件（[`ListCoachAcceptanceChangesQuery`]）の日付を両方省けば今までどおりすべて。どちらかがあれば、変えた日付（日本時間。
/// [`calendar_day`] で区切る）が `from` から `to` まで（両端を含む）の記録（domain の
/// [`CoachAcceptanceChange::is_changed_in`]）だけに絞る（[`CoachAcceptanceRepository::changes_between`]）。`from` だけなら
/// `from` から、`to` だけなら `to` までの 7 日で、期間の規則（`to` が `from` より前でない・31 日まで）も本部が送ったお知らせの
/// 一覧と同じ（[`calendar_day::period_ending`] と domain の [`domain::scheduling::BookingPeriod`]）。並べ方は絞っても変わらない。
pub struct ListCoachAcceptanceChanges<R, I> {
    acceptances: R,
    coaches: I,
}

impl<R, I> ListCoachAcceptanceChanges<R, I>
where
    R: CoachAcceptanceRepository,
    I: CoachIntroductionLookup,
{
    pub fn new(acceptances: R, coaches: I) -> Self {
        Self {
            acceptances,
            coaches,
        }
    }

    /// 期間を作る前に問い合わせない（期間の違反は、コーチがいるかを確かめずに返す）。
    pub async fn execute(
        &self,
        coach_id: AccountId,
        query: ListCoachAcceptanceChangesQuery,
    ) -> Result<Option<Vec<CoachAcceptanceChange>>, ListCoachAcceptanceChangesError> {
        let period = match (query.from, query.to) {
            (None, None) => None,
            (from, to) => {
                let (_, _, period) = calendar_day::period_ending(from, to, || {
                    unreachable!("from か to のどちらかがあるので今日は読まない")
                })?;
                Some(period)
            }
        };
        if !coach_exists(&self.coaches, coach_id).await? {
            return Ok(None);
        }
        let changes = match period {
            None => self.acceptances.changes(coach_id).await?,
            Some(period) => {
                let mut changes = self
                    .acceptances
                    .changes_between(coach_id, period.starts_at(), period.ends_at())
                    .await?;
                changes.retain(|change| change.is_changed_in(&period));
                changes
            }
        };
        Ok(Some(changes))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::Sport,
        identity::{Account, Email, Role},
        profile::DisplayName,
        scheduling::{CoachAcceptanceStatus, CoachSuspensionReason, SchedulingError},
    };
    use time::{Date, Month};

    use super::*;
    use crate::{
        identity::AccountRepository,
        in_memory::{InMemoryAccountRepository, InMemoryCoachAcceptanceRepository},
        profile::ProfileCoachIntroductionLookup,
    };

    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const HEADQUARTERS: AccountId = AccountId::from_u128(2);
    const OTHER_HEADQUARTERS: AccountId = AccountId::from_u128(3);

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn nov(day: u8) -> Date {
        Date::from_calendar_date(2026, Month::November, day).unwrap()
    }

    fn query(from: Option<Date>, to: Option<Date>) -> ListCoachAcceptanceChangesQuery {
        ListCoachAcceptanceChangesQuery { from, to }
    }

    /// `COACH` の受付を `times` の時刻に交互に止めて・再開した保存先（最初は止める）。
    async fn toggled_at(times: &[SystemTime]) -> InMemoryCoachAcceptanceRepository {
        let acceptances = InMemoryCoachAcceptanceRepository::default();
        for (i, &time) in times.iter().enumerate() {
            acceptances
                .update(COACH, |a| {
                    if i % 2 == 0 {
                        a.suspend(HEADQUARTERS, CoachSuspensionReason::default(), time)
                    } else {
                        a.resume(HEADQUARTERS, time)
                    }
                })
                .await
                .unwrap();
        }
        acceptances
    }

    fn changed_ats(changes: &[CoachAcceptanceChange]) -> Vec<SystemTime> {
        changes
            .iter()
            .map(CoachAcceptanceChange::changed_at)
            .collect()
    }

    fn reason(text: &str) -> CoachSuspensionReason {
        CoachSuspensionReason::new(text).unwrap()
    }

    type UseCase = ListCoachAcceptanceChanges<
        InMemoryCoachAcceptanceRepository,
        ProfileCoachIntroductionLookup<InMemoryAccountRepository>,
    >;

    async fn use_case(acceptances: InMemoryCoachAcceptanceRepository) -> UseCase {
        let accounts = InMemoryAccountRepository::default();
        for (id, role) in [
            (COACH, Role::Coach),
            (OTHER_COACH, Role::Coach),
            (STUDENT, Role::Student),
        ] {
            accounts
                .insert(
                    Account::new(
                        id,
                        Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
                        DisplayName::new("たなか").unwrap(),
                        role,
                        [Sport::Tennis],
                        SystemTime::UNIX_EPOCH,
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
        }
        ListCoachAcceptanceChanges::new(acceptances, ProfileCoachIntroductionLookup::new(accounts))
    }

    #[tokio::test]
    async fn lists_the_changes_of_the_coach_newest_first_with_the_reasons() {
        let acceptances = InMemoryCoachAcceptanceRepository::default();
        acceptances
            .update(COACH, |a| {
                a.suspend(HEADQUARTERS, reason("無断欠席が続いた"), at(10))
            })
            .await
            .unwrap();
        acceptances
            .update(OTHER_COACH, |a| {
                a.suspend(HEADQUARTERS, CoachSuspensionReason::default(), at(15))
            })
            .await
            .unwrap();
        acceptances
            .update(COACH, |a| a.resume(OTHER_HEADQUARTERS, at(20)))
            .await
            .unwrap();
        // 同じ時刻なら後に保存した方が先
        acceptances
            .update(COACH, |a| {
                a.suspend(HEADQUARTERS, CoachSuspensionReason::default(), at(20))
            })
            .await
            .unwrap();

        let changes = use_case(acceptances)
            .await
            .execute(COACH, ListCoachAcceptanceChangesQuery::default())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            changes,
            vec![
                CoachAcceptanceChange::restore(
                    COACH,
                    CoachAcceptanceStatus::Suspended,
                    Some(CoachSuspensionReason::default()),
                    HEADQUARTERS,
                    at(20)
                ),
                CoachAcceptanceChange::restore(
                    COACH,
                    CoachAcceptanceStatus::Accepting,
                    None,
                    OTHER_HEADQUARTERS,
                    at(20)
                ),
                CoachAcceptanceChange::restore(
                    COACH,
                    CoachAcceptanceStatus::Suspended,
                    Some(reason("無断欠席が続いた")),
                    HEADQUARTERS,
                    at(10)
                ),
            ]
        );
    }

    #[tokio::test]
    async fn a_coach_never_suspended_has_no_changes() {
        let changes = use_case(InMemoryCoachAcceptanceRepository::default())
            .await
            .execute(COACH, ListCoachAcceptanceChangesQuery::default())
            .await
            .unwrap();
        assert_eq!(changes, Some(Vec::new()));
    }

    #[tokio::test]
    async fn a_non_coach_or_missing_account_is_none() {
        let use_case = use_case(InMemoryCoachAcceptanceRepository::default()).await;
        for id in [STUDENT, AccountId::from_u128(99)] {
            assert_eq!(
                use_case
                    .execute(id, ListCoachAcceptanceChangesQuery::default())
                    .await
                    .unwrap(),
                None
            );
        }
    }

    #[tokio::test]
    async fn from_and_to_narrow_to_the_changed_dates_in_japan_including_both_ends() {
        let one_nano = Duration::from_nanos(1);
        let times = [
            calendar_day::start(nov(1)) - one_nano,
            calendar_day::start(nov(1)),
            calendar_day::end(nov(3)) - one_nano,
            calendar_day::end(nov(3)),
        ];
        let use_case = use_case(toggled_at(&times).await).await;

        let changes = use_case
            .execute(COACH, query(Some(nov(1)), Some(nov(3))))
            .await
            .unwrap()
            .unwrap();
        // 新しい順のまま。前の日の終わりと次の日の始まりは入らない
        assert_eq!(changed_ats(&changes), vec![times[2], times[1]]);

        // 両方省けば今までどおりすべて
        let all = use_case
            .execute(COACH, ListCoachAcceptanceChangesQuery::default())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(all.len(), 4);
    }

    #[tokio::test]
    async fn only_from_or_only_to_is_seven_days() {
        let times = [
            calendar_day::start(nov(1)),
            calendar_day::start(nov(7)),
            calendar_day::start(nov(8)),
        ];
        let use_case = use_case(toggled_at(&times).await).await;

        let from_only = use_case
            .execute(COACH, query(Some(nov(1)), None))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(changed_ats(&from_only), vec![times[1], times[0]]);

        let to_only = use_case
            .execute(COACH, query(None, Some(nov(8))))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(changed_ats(&to_only), vec![times[2], times[1]]);
    }

    #[tokio::test]
    async fn no_changes_in_the_period_is_an_empty_list() {
        let use_case = use_case(toggled_at(&[calendar_day::start(nov(1))]).await).await;
        let changes = use_case
            .execute(COACH, query(Some(nov(2)), Some(nov(3))))
            .await
            .unwrap();
        assert_eq!(changes, Some(Vec::new()));
    }

    #[tokio::test]
    async fn a_reversed_or_too_long_period_is_invalid_before_looking_up_the_coach() {
        let use_case = use_case(InMemoryCoachAcceptanceRepository::default()).await;
        let december = |day| Date::from_calendar_date(2026, Month::December, day).unwrap();
        for id in [COACH, STUDENT, AccountId::from_u128(99)] {
            for (from, to, expected) in [
                (
                    nov(3),
                    nov(2),
                    SchedulingError::BookingPeriodEndsBeforeItStarts,
                ),
                (
                    nov(1),
                    december(2),
                    SchedulingError::BookingPeriodTooLong { max_days: 31 },
                ),
            ] {
                let error = use_case
                    .execute(id, query(Some(from), Some(to)))
                    .await
                    .unwrap_err();
                assert!(
                    matches!(error, ListCoachAcceptanceChangesError::Invalid(e) if e == expected),
                    "{from}〜{to}"
                );
            }
        }
        // 31 日までは通る
        assert!(
            use_case
                .execute(COACH, query(Some(nov(1)), Some(december(1))))
                .await
                .is_ok()
        );
    }
}
