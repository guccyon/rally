use std::collections::BTreeSet;

use domain::{
    catalog::{LessonMenuId, Sport},
    identity::AccountId,
    profile::CoachSportFilter,
    scheduling::{Slot, SlotSupply},
};
use time::Date;

use super::{
    BookingRepository, ListBookingsError, SlotCoachAccountLookup, SlotSupplyReport, calendar_day,
};
use crate::clock::Clock;

/// スポーツごとの予約枠の需給のユースケース（admin-web の `getSlotSupply`。本部だけが呼ぶ）。
///
/// 期間に開始する枠と、それらへの予約を読み、スポーツ 6 種類すべてについて [`SlotSupply::tally`] で数える。数え方と
/// 判定は domain の [`SlotSupply`] だけ。期間は、日付（日本時間）の `from`・`to` を両方省けば今までどおり
/// [`SlotSupply::default_period`]（現在時刻までの [`SlotSupply::WINDOW`]）。どちらかを付ければ予約の一覧
/// （[`super::ListBookings`]）と同じ規則（[`calendar_day::period`]。`from` を省けば今日、`to` を省けば `from` から 7 日目、
/// 両端を含み 31 日まで）で、違反は [`ListBookingsError::Invalid`]。`coach_id` があれば、そのコーチの枠とそれらへの予約だけを
/// 同じ数え方で数える（いないコーチ・コーチでない ID ならどのスポーツも 0。規則は domain の
/// [`Slot::matches_coach`]。予約の一覧の `coach_id` と同じ扱い）。`sport` があれば、残った枠のコーチのアカウント
/// （[`SlotCoachAccountLookup`]）を引き、その競技を提供するコーチの枠とそれらへの予約だけを同じ数え方で数える（規則は
/// domain の [`Slot::matches_coach_sport`]。コーチの競技の決め方は [`CoachSportFilter`] の 1 か所。`coach_id` と組み合わせ
/// られ、結果はどちらもスポーツ 6 種類すべて）。`lesson_menu_id` があれば、そのレッスンメニューに結びついた枠とそれらへの
/// 予約だけを同じ数え方で数える（メニューのない枠は数えない。ないメニューの ID ならどのスポーツも 0。規則は domain の
/// [`Slot::matches_lesson_menu`]。予約の一覧の `lesson_menu_id` と同じ扱い。期間・`coach_id`・`sport` と組み合わせられる）。
pub struct GetSlotSupply<B, A, C> {
    bookings: B,
    coach_accounts: A,
    clock: C,
}

/// 予約枠の需給の条件。どれも省ける。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GetSlotSupplyQuery {
    /// 期間の始まりの日付（日本時間。含む）
    pub from: Option<Date>,
    /// 期間の終わりの日付（日本時間。含む）
    pub to: Option<Date>,
    /// あればこのコーチの枠（とそれらへの予約）だけを数える
    pub coach_id: Option<AccountId>,
    /// あればこの競技を提供するコーチの枠（とそれらへの予約）だけを数える
    pub sport: Option<Sport>,
    /// あればこのレッスンメニューに結びついた枠（とそれらへの予約）だけを数える
    pub lesson_menu_id: Option<LessonMenuId>,
}

impl<B, A, C> GetSlotSupply<B, A, C>
where
    B: BookingRepository,
    A: SlotCoachAccountLookup,
    C: Clock,
{
    pub fn new(bookings: B, coach_accounts: A, clock: C) -> Self {
        Self {
            bookings,
            coach_accounts,
            clock,
        }
    }

    pub async fn execute(
        &self,
        query: GetSlotSupplyQuery,
    ) -> Result<SlotSupplyReport, ListBookingsError> {
        let now = self.clock.now();
        let (dates, period) = if query.from.is_none() && query.to.is_none() {
            (None, SlotSupply::default_period(now))
        } else {
            let (from, to, period) = calendar_day::period(query.from, query.to, now)?;
            (Some((from, to)), period)
        };
        let (mut slots, bookings) = self
            .bookings
            .slots_starting_between(period.starts_at(), period.ends_at())
            .await?;
        // 絞った枠にない枠への予約は SlotSupply::tally が数えないので、予約はそのまま渡す
        slots.retain(|slot| {
            slot.matches_coach(query.coach_id) && slot.matches_lesson_menu(query.lesson_menu_id)
        });
        let sport = CoachSportFilter::new(query.sport);
        if sport.sport().is_some() && !slots.is_empty() {
            let coach_ids: BTreeSet<AccountId> = slots.iter().map(Slot::coach_id).collect();
            let coaches = self
                .coach_accounts
                .find_slot_coach_accounts(&coach_ids)
                .await?;
            slots.retain(|slot| slot.matches_coach_sport(sport, coaches.get(&slot.coach_id())));
        }
        Ok(SlotSupplyReport {
            window_start: period.starts_at(),
            window_end: period.ends_at(),
            dates,
            sports: Sport::ALL
                .into_iter()
                .map(|sport| SlotSupply::tally(sport, &slots, &bookings))
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        identity::{Account, AccountId, Email, Role},
        profile::DisplayName,
        scheduling::{Booking, SlotId, SlotStatus, SlotSupplyStatus},
    };

    use domain::scheduling::SchedulingError;
    use time::Duration as Days;

    use super::*;
    use crate::{
        identity::AccountRepository,
        in_memory::{
            FixedClock, InMemoryAccountRepository, InMemoryBookingRepository,
            InMemorySlotRepository,
        },
        profile::ProfileCoachAccountLookup,
    };

    const DAY: u64 = 24 * 3600;
    /// 現在時刻（100 日目の 0 時）
    const NOW: Duration = Duration::from_secs(100 * DAY);

    fn no_accounts() -> ProfileCoachAccountLookup<InMemoryAccountRepository> {
        ProfileCoachAccountLookup::new(InMemoryAccountRepository::default())
    }

    /// `accounts`（ID・役割・提供スポーツ）を入れたアカウントの保存先から枠のコーチのアカウントを引く。
    async fn accounts_of(
        accounts: &[(AccountId, Role, &[Sport])],
    ) -> ProfileCoachAccountLookup<InMemoryAccountRepository> {
        let repository = InMemoryAccountRepository::default();
        for (id, role, sports) in accounts {
            repository
                .insert(
                    Account::new(
                        *id,
                        Email::new(format!("{}@example.com", id.as_u128())).unwrap(),
                        DisplayName::new("テスト").unwrap(),
                        *role,
                        sports.iter().copied(),
                        SystemTime::UNIX_EPOCH,
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
        }
        ProfileCoachAccountLookup::new(repository)
    }

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn slot(id: u128, sport: Sport, starts_at: SystemTime) -> Slot {
        slot_of(id, AccountId::from_u128(1000 + id), sport, starts_at)
    }

    fn slot_of(id: u128, coach: AccountId, sport: Sport, starts_at: SystemTime) -> Slot {
        Slot::restore(
            SlotId::from_u128(id),
            coach,
            sport,
            starts_at,
            starts_at + Duration::from_secs(3600),
            SlotStatus::Open,
            domain::scheduling::SlotSeats::default(),
        )
        .unwrap()
    }

    /// 枠も予約しようとした生徒も 0 のスポーツの判定。ゴルフは対象外、それ以外は枠も予約もない。
    fn zero_status(sport: Sport) -> SlotSupplyStatus {
        if sport == Sport::Golf {
            SlotSupplyStatus::NotApplicable
        } else {
            SlotSupplyStatus::NoActivity
        }
    }

    async fn book(bookings: &InMemoryBookingRepository, student: u128, slot: u128) {
        let student = AccountId::from_u128(student);
        bookings
            .reserve(SlotId::from_u128(slot), student, |slot, existing| {
                Booking::reserve(bookings.next_id(), student, false, slot, existing, at(0))
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn counts_slots_starting_in_the_last_thirty_days_for_every_sport() {
        let now = SystemTime::UNIX_EPOCH + NOW;
        let window_start = now - Duration::from_secs(30 * DAY);
        let slots = InMemorySlotRepository::with_slots([
            // 期間の始まりちょうど（含む）と終わりの直前
            slot(1, Sport::Tennis, window_start),
            slot(2, Sport::Tennis, now - Duration::from_secs(1)),
            // 期間の外（始まりの直前と、現在時刻ちょうど）
            slot(3, Sport::Tennis, window_start - Duration::from_secs(1)),
            slot(4, Sport::Tennis, now),
            slot(5, Sport::Soccer, now - Duration::from_secs(DAY)),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        book(&bookings, 7, 1).await;
        book(&bookings, 8, 2).await;
        // 期間の外の枠への予約は数えない
        book(&bookings, 9, 3).await;
        book(&bookings, 10, 4).await;

        let report = GetSlotSupply::new(bookings, no_accounts(), FixedClock(now))
            .execute(GetSlotSupplyQuery::default())
            .await
            .unwrap();
        assert_eq!(
            (report.window_start, report.window_end),
            (window_start, now)
        );
        assert_eq!(report.dates, None);
        assert_eq!(
            report.sports,
            [
                SlotSupply::new(Sport::Tennis, 2, 2),
                SlotSupply::new(Sport::Golf, 0, 0),
                SlotSupply::new(Sport::Baseball, 0, 0),
                SlotSupply::new(Sport::Soccer, 1, 0),
                SlotSupply::new(Sport::Basketball, 0, 0),
                SlotSupply::new(Sport::Swimming, 0, 0),
            ]
        );
        let statuses: Vec<_> = report.sports.iter().map(SlotSupply::status).collect();
        assert_eq!(
            statuses,
            [
                SlotSupplyStatus::Insufficient,
                SlotSupplyStatus::NotApplicable,
                SlotSupplyStatus::NoActivity,
                SlotSupplyStatus::Sufficient,
                SlotSupplyStatus::NoActivity,
                SlotSupplyStatus::NoActivity,
            ]
        );
    }

    #[tokio::test]
    async fn counts_slots_starting_on_the_dates_in_japan_both_ends_included() {
        let now = SystemTime::UNIX_EPOCH + NOW;
        // 来週の 7 日（今日の日本時間の日付の 7 日後から 13 日後まで）
        let from = calendar_day::of(now) + Days::days(7);
        let to = from + Days::days(6);
        let (start, end) = (calendar_day::start(from), calendar_day::end(to));
        let slots = InMemorySlotRepository::with_slots([
            // 期間の始まり（from の日本時間 0 時）ちょうどと、終わり（to の翌日 0 時）の直前
            slot(1, Sport::Tennis, start),
            slot(2, Sport::Tennis, end - Duration::from_secs(1)),
            // 期間の外（始まりの直前と、終わりちょうど）。今日の枠も入らない
            slot(3, Sport::Tennis, start - Duration::from_secs(1)),
            slot(4, Sport::Tennis, end),
            slot(5, Sport::Tennis, now - Duration::from_secs(DAY)),
            slot(6, Sport::Swimming, start + Duration::from_secs(DAY)),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        book(&bookings, 7, 1).await;
        book(&bookings, 8, 4).await;
        book(&bookings, 9, 5).await;

        let report = GetSlotSupply::new(bookings, no_accounts(), FixedClock(now))
            .execute(GetSlotSupplyQuery {
                from: Some(from),
                to: Some(to),
                coach_id: None,
                sport: None,
                lesson_menu_id: None,
            })
            .await
            .unwrap();
        assert_eq!((report.window_start, report.window_end), (start, end));
        assert_eq!(report.dates, Some((from, to)));
        assert_eq!(report.sports[0], SlotSupply::new(Sport::Tennis, 2, 1));
        assert_eq!(report.sports[5], SlotSupply::new(Sport::Swimming, 1, 0));
        assert_eq!(report.sports.len(), Sport::ALL.len());
    }

    #[tokio::test]
    async fn with_coach_counts_only_the_slots_of_that_coach_and_their_bookings() {
        let now = SystemTime::UNIX_EPOCH + NOW;
        let (coach, other) = (AccountId::from_u128(500), AccountId::from_u128(501));
        let in_window = |days: u64| now - Duration::from_secs(days * DAY);
        let slots = InMemorySlotRepository::with_slots([
            slot_of(1, coach, Sport::Tennis, in_window(1)),
            slot_of(2, coach, Sport::Tennis, in_window(2)),
            slot_of(3, coach, Sport::Swimming, in_window(3)),
            // 別のコーチの枠と、期間の外のそのコーチの枠
            slot_of(4, other, Sport::Tennis, in_window(1)),
            slot_of(5, other, Sport::Soccer, in_window(1)),
            slot_of(6, coach, Sport::Tennis, now),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        book(&bookings, 7, 1).await;
        // 別のコーチの枠への予約と、期間の外の枠への予約は数えない
        book(&bookings, 8, 4).await;
        book(&bookings, 9, 5).await;
        book(&bookings, 10, 6).await;

        let supply = GetSlotSupply::new(bookings, no_accounts(), FixedClock(now));
        let report = supply
            .execute(GetSlotSupplyQuery {
                coach_id: Some(coach),
                ..GetSlotSupplyQuery::default()
            })
            .await
            .unwrap();
        // 期間はコーチを付けても変わらない
        assert_eq!(
            (report.window_start, report.window_end, report.dates),
            (now - SlotSupply::WINDOW, now, None)
        );
        assert_eq!(
            report.sports,
            [
                SlotSupply::new(Sport::Tennis, 2, 1),
                SlotSupply::new(Sport::Golf, 0, 0),
                SlotSupply::new(Sport::Baseball, 0, 0),
                SlotSupply::new(Sport::Soccer, 0, 0),
                SlotSupply::new(Sport::Basketball, 0, 0),
                SlotSupply::new(Sport::Swimming, 1, 0),
            ]
        );
        // 付けなければ今までどおりすべてのコーチ
        let all = supply.execute(GetSlotSupplyQuery::default()).await.unwrap();
        assert_eq!(all.sports[0], SlotSupply::new(Sport::Tennis, 3, 2));
        assert_eq!(all.sports[3], SlotSupply::new(Sport::Soccer, 1, 1));
    }

    #[tokio::test]
    async fn with_coach_and_dates_counts_that_coach_in_the_days() {
        let now = SystemTime::UNIX_EPOCH + NOW;
        let coach = AccountId::from_u128(500);
        let from = calendar_day::of(now) + Days::days(7);
        let to = from + Days::days(6);
        let start = calendar_day::start(from);
        let slots = InMemorySlotRepository::with_slots([
            slot_of(1, coach, Sport::Tennis, start),
            slot_of(2, AccountId::from_u128(501), Sport::Tennis, start),
            // そのコーチの、期間の外の枠
            slot_of(3, coach, Sport::Tennis, start - Duration::from_secs(1)),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        book(&bookings, 7, 1).await;
        book(&bookings, 8, 2).await;
        book(&bookings, 9, 3).await;

        let report = GetSlotSupply::new(bookings, no_accounts(), FixedClock(now))
            .execute(GetSlotSupplyQuery {
                from: Some(from),
                to: Some(to),
                coach_id: Some(coach),
                sport: None,
                lesson_menu_id: None,
            })
            .await
            .unwrap();
        assert_eq!(report.dates, Some((from, to)));
        assert_eq!(report.sports[0], SlotSupply::new(Sport::Tennis, 1, 1));
    }

    #[tokio::test]
    async fn with_unknown_coach_every_sport_is_zero() {
        let now = SystemTime::UNIX_EPOCH + NOW;
        let slots = InMemorySlotRepository::with_slots([slot(
            1,
            Sport::Tennis,
            now - Duration::from_secs(DAY),
        )]);
        let bookings = InMemoryBookingRepository::new(slots);
        book(&bookings, 7, 1).await;

        let report = GetSlotSupply::new(bookings, no_accounts(), FixedClock(now))
            .execute(GetSlotSupplyQuery {
                // ない ID と、予約した生徒（コーチでない ID）
                coach_id: Some(AccountId::from_u128(99)),
                ..GetSlotSupplyQuery::default()
            })
            .await
            .unwrap();
        assert_eq!(report.sports.len(), Sport::ALL.len());
        assert!(
            report
                .sports
                .iter()
                .all(|supply| supply.published_slots() == 0 && supply.booking_students() == 0)
        );
        // 0 件はゴルフが対象外、それ以外が枠も予約もない
        assert!(
            report
                .sports
                .iter()
                .all(|supply| supply.status() == zero_status(supply.sport()))
        );
    }

    #[tokio::test]
    async fn a_missing_date_is_decided_like_list_bookings() {
        let now = SystemTime::UNIX_EPOCH + NOW;
        let today = calendar_day::of(now);
        let supply = GetSlotSupply::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            no_accounts(),
            FixedClock(now),
        );
        let dates = |from, to| {
            let supply = &supply;
            async move {
                supply
                    .execute(GetSlotSupplyQuery {
                        from,
                        to,
                        coach_id: None,
                        sport: None,
                        lesson_menu_id: None,
                    })
                    .await
                    .map(|report| (report.dates, report.window_start, report.window_end))
            }
        };
        // from だけなら from から 7 日目まで、to だけなら今日から to まで
        let from = today + Days::days(10);
        let (got, start, end) = dates(Some(from), None).await.unwrap();
        assert_eq!(got, Some((from, from + Days::days(6))));
        assert_eq!(start, calendar_day::start(from));
        assert_eq!(end, calendar_day::end(from + Days::days(6)));
        let to = today + Days::days(30);
        let (got, ..) = dates(None, Some(to)).await.unwrap();
        assert_eq!(got, Some((today, to)));
    }

    #[tokio::test]
    async fn dates_breaking_the_rule_are_invalid() {
        let now = SystemTime::UNIX_EPOCH + NOW;
        let today = calendar_day::of(now);
        let supply = GetSlotSupply::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            no_accounts(),
            FixedClock(now),
        );
        let invalid = |from, to| {
            let supply = &supply;
            async move {
                match supply
                    .execute(GetSlotSupplyQuery {
                        from,
                        to,
                        coach_id: None,
                        sport: None,
                        lesson_menu_id: None,
                    })
                    .await
                {
                    Err(ListBookingsError::Invalid(error)) => error,
                    other => panic!("期間の違反にならない: {other:?}"),
                }
            }
        };
        // 31 日（両端を含む）までは通り、32 日は通らない
        assert!(
            supply
                .execute(GetSlotSupplyQuery {
                    from: Some(today),
                    to: Some(today + Days::days(30)),
                    coach_id: None,
                    sport: None,
                    lesson_menu_id: None,
                })
                .await
                .is_ok()
        );
        assert_eq!(
            invalid(Some(today), Some(today + Days::days(31))).await,
            SchedulingError::BookingPeriodTooLong { max_days: 31 }
        );
        assert_eq!(
            invalid(Some(today), Some(today - Days::days(1))).await,
            SchedulingError::BookingPeriodEndsBeforeItStarts
        );
        // to だけで今日より前は from（今日）より前
        assert_eq!(
            invalid(None, Some(today - Days::days(1))).await,
            SchedulingError::BookingPeriodEndsBeforeItStarts
        );
    }

    #[tokio::test]
    async fn with_sport_counts_only_the_slots_of_coaches_offering_it() {
        let now = SystemTime::UNIX_EPOCH + NOW;
        // たなかはテニスとサッカー、すずきはテニスだけ、さとうは水泳だけを提供する。生徒 9 の枠はない
        let (tanaka, suzuki, sato) = (
            AccountId::from_u128(500),
            AccountId::from_u128(501),
            AccountId::from_u128(502),
        );
        let accounts = accounts_of(&[
            (tanaka, Role::Coach, &[Sport::Tennis, Sport::Soccer]),
            (suzuki, Role::Coach, &[Sport::Tennis]),
            (sato, Role::Coach, &[Sport::Swimming]),
            (AccountId::from_u128(9), Role::Student, &[Sport::Soccer]),
        ])
        .await;
        let in_window = |days: u64| now - Duration::from_secs(days * DAY);
        let slots = InMemorySlotRepository::with_slots([
            slot_of(1, tanaka, Sport::Tennis, in_window(1)),
            slot_of(2, tanaka, Sport::Soccer, in_window(2)),
            slot_of(3, suzuki, Sport::Tennis, in_window(1)),
            slot_of(4, sato, Sport::Swimming, in_window(1)),
            // アカウントのないコーチの枠と、期間の外のたなかの枠
            slot_of(5, AccountId::from_u128(600), Sport::Soccer, in_window(1)),
            slot_of(6, tanaka, Sport::Soccer, now),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        book(&bookings, 7, 1).await;
        book(&bookings, 8, 2).await;
        book(&bookings, 9, 3).await;
        book(&bookings, 10, 5).await;
        book(&bookings, 11, 6).await;

        let supply = GetSlotSupply::new(bookings, accounts, FixedClock(now));
        let with_sport = |sport, coach_id| {
            let supply = &supply;
            async move {
                supply
                    .execute(GetSlotSupplyQuery {
                        sport: Some(sport),
                        coach_id,
                        ..GetSlotSupplyQuery::default()
                    })
                    .await
                    .unwrap()
            }
        };
        // サッカーを提供するのはたなかだけ。たなかの枠はテニスの枠も数える（枠のスポーツでは絞らない）
        let report = with_sport(Sport::Soccer, None).await;
        assert_eq!(
            (report.window_start, report.window_end, report.dates),
            (now - SlotSupply::WINDOW, now, None),
            "期間は競技を付けても変わらない"
        );
        assert_eq!(
            report.sports,
            [
                SlotSupply::new(Sport::Tennis, 1, 1),
                SlotSupply::new(Sport::Golf, 0, 0),
                SlotSupply::new(Sport::Baseball, 0, 0),
                SlotSupply::new(Sport::Soccer, 1, 1),
                SlotSupply::new(Sport::Basketball, 0, 0),
                SlotSupply::new(Sport::Swimming, 0, 0),
            ]
        );
        // テニスはたなかとすずき
        let report = with_sport(Sport::Tennis, None).await;
        assert_eq!(report.sports[0], SlotSupply::new(Sport::Tennis, 2, 2));
        assert_eq!(report.sports[3], SlotSupply::new(Sport::Soccer, 1, 1));
        assert_eq!(report.sports[5], SlotSupply::new(Sport::Swimming, 0, 0));
        // 提供するコーチのいない競技はどのスポーツも 0
        let report = with_sport(Sport::Golf, None).await;
        assert_eq!(report.sports.len(), Sport::ALL.len());
        assert!(
            report
                .sports
                .iter()
                .all(|supply| { supply.published_slots() == 0 && supply.booking_students() == 0 })
        );
        // コーチと組み合わせる。すずきはサッカーを提供しないので 0、テニスならすずきの枠だけ
        let report = with_sport(Sport::Soccer, Some(suzuki)).await;
        assert!(
            report
                .sports
                .iter()
                .all(|supply| supply.published_slots() == 0)
        );
        let report = with_sport(Sport::Tennis, Some(suzuki)).await;
        assert_eq!(report.sports[0], SlotSupply::new(Sport::Tennis, 1, 1));
        assert_eq!(report.sports[3], SlotSupply::new(Sport::Soccer, 0, 0));
        // 付けなければ今までどおりすべてのコーチ（アカウントのないコーチの枠も数える）
        let all = supply.execute(GetSlotSupplyQuery::default()).await.unwrap();
        assert_eq!(all.sports[0], SlotSupply::new(Sport::Tennis, 2, 2));
        assert_eq!(all.sports[3], SlotSupply::new(Sport::Soccer, 2, 2));
        assert_eq!(all.sports[5], SlotSupply::new(Sport::Swimming, 1, 0));
    }

    #[tokio::test]
    async fn with_sport_and_dates_counts_the_coaches_offering_it_in_the_days() {
        let now = SystemTime::UNIX_EPOCH + NOW;
        let (tennis, soccer) = (AccountId::from_u128(500), AccountId::from_u128(501));
        let accounts = accounts_of(&[
            (tennis, Role::Coach, &[Sport::Tennis]),
            (soccer, Role::Coach, &[Sport::Soccer]),
        ])
        .await;
        let from = calendar_day::of(now) + Days::days(7);
        let to = from + Days::days(6);
        let start = calendar_day::start(from);
        let slots = InMemorySlotRepository::with_slots([
            slot_of(1, tennis, Sport::Tennis, start),
            slot_of(2, soccer, Sport::Soccer, start),
            // テニスのコーチの、期間の外の枠
            slot_of(3, tennis, Sport::Tennis, start - Duration::from_secs(1)),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        book(&bookings, 7, 1).await;
        book(&bookings, 8, 2).await;
        book(&bookings, 9, 3).await;

        let report = GetSlotSupply::new(bookings, accounts, FixedClock(now))
            .execute(GetSlotSupplyQuery {
                from: Some(from),
                to: Some(to),
                coach_id: None,
                sport: Some(Sport::Tennis),
                lesson_menu_id: None,
            })
            .await
            .unwrap();
        assert_eq!(report.dates, Some((from, to)));
        assert_eq!(report.sports[0], SlotSupply::new(Sport::Tennis, 1, 1));
        assert_eq!(report.sports[3], SlotSupply::new(Sport::Soccer, 0, 0));
    }

    #[tokio::test]
    async fn with_lesson_menu_counts_only_the_slots_bound_to_it_and_their_bookings() {
        let now = SystemTime::UNIX_EPOCH + NOW;
        let (menu, other_menu) = (LessonMenuId::from_u128(50), LessonMenuId::from_u128(51));
        // たなかはテニスとサッカー、すずきはテニスだけを提供する
        let (tanaka, suzuki) = (AccountId::from_u128(500), AccountId::from_u128(501));
        let accounts = accounts_of(&[
            (tanaka, Role::Coach, &[Sport::Tennis, Sport::Soccer]),
            (suzuki, Role::Coach, &[Sport::Tennis]),
        ])
        .await;
        let in_window = |days: u64| now - Duration::from_secs(days * DAY);
        let with_menu = |slot: Slot, menu| slot.with_restored_lesson_menu(Some(menu));
        let slots = InMemorySlotRepository::with_slots([
            with_menu(slot_of(1, tanaka, Sport::Tennis, in_window(1)), menu),
            with_menu(slot_of(2, tanaka, Sport::Tennis, in_window(2)), menu),
            with_menu(slot_of(3, suzuki, Sport::Tennis, in_window(1)), menu),
            // 別のメニューの枠、メニューのない枠、期間の外のそのメニューの枠
            with_menu(slot_of(4, tanaka, Sport::Soccer, in_window(1)), other_menu),
            slot_of(5, tanaka, Sport::Tennis, in_window(1)),
            with_menu(slot_of(6, tanaka, Sport::Tennis, now), menu),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        book(&bookings, 7, 1).await;
        book(&bookings, 8, 3).await;
        // 別のメニューの枠・メニューのない枠・期間の外の枠への予約は数えない
        book(&bookings, 9, 4).await;
        book(&bookings, 10, 5).await;
        book(&bookings, 11, 6).await;

        let supply = GetSlotSupply::new(bookings, accounts, FixedClock(now));
        let with_lesson_menu = |lesson_menu_id, coach_id, sport| {
            let supply = &supply;
            async move {
                supply
                    .execute(GetSlotSupplyQuery {
                        lesson_menu_id: Some(lesson_menu_id),
                        coach_id,
                        sport,
                        ..GetSlotSupplyQuery::default()
                    })
                    .await
                    .unwrap()
            }
        };
        let report = with_lesson_menu(menu, None, None).await;
        assert_eq!(
            (report.window_start, report.window_end, report.dates),
            (now - SlotSupply::WINDOW, now, None),
            "期間はメニューを付けても変わらない"
        );
        assert_eq!(
            report.sports,
            [
                SlotSupply::new(Sport::Tennis, 3, 2),
                SlotSupply::new(Sport::Golf, 0, 0),
                SlotSupply::new(Sport::Baseball, 0, 0),
                SlotSupply::new(Sport::Soccer, 0, 0),
                SlotSupply::new(Sport::Basketball, 0, 0),
                SlotSupply::new(Sport::Swimming, 0, 0),
            ]
        );
        // コーチ・競技と組み合わせる
        let report = with_lesson_menu(menu, Some(suzuki), None).await;
        assert_eq!(report.sports[0], SlotSupply::new(Sport::Tennis, 1, 1));
        let report = with_lesson_menu(menu, None, Some(Sport::Soccer)).await;
        assert_eq!(report.sports[0], SlotSupply::new(Sport::Tennis, 2, 1));
        let report = with_lesson_menu(other_menu, Some(suzuki), None).await;
        assert!(
            report
                .sports
                .iter()
                .all(|supply| supply.published_slots() == 0 && supply.booking_students() == 0)
        );
        // ないメニューの ID はどのスポーツも 0（ゴルフは対象外、それ以外は枠も予約もない）
        let report = with_lesson_menu(LessonMenuId::from_u128(99), None, None).await;
        assert_eq!(report.sports.len(), Sport::ALL.len());
        assert!(report.sports.iter().all(|supply| {
            supply.published_slots() == 0
                && supply.booking_students() == 0
                && supply.status() == zero_status(supply.sport())
        }));
        // 付けなければ今までどおりメニューを問わない（メニューのない枠も数える）
        let all = supply.execute(GetSlotSupplyQuery::default()).await.unwrap();
        assert_eq!(all.sports[0], SlotSupply::new(Sport::Tennis, 4, 3));
        assert_eq!(all.sports[3], SlotSupply::new(Sport::Soccer, 1, 1));
    }

    #[tokio::test]
    async fn with_lesson_menu_and_dates_counts_the_slots_of_that_menu_in_the_days() {
        let now = SystemTime::UNIX_EPOCH + NOW;
        let menu = LessonMenuId::from_u128(50);
        let from = calendar_day::of(now) + Days::days(7);
        let to = from + Days::days(6);
        let start = calendar_day::start(from);
        let slots = InMemorySlotRepository::with_slots([
            slot(1, Sport::Tennis, start).with_restored_lesson_menu(Some(menu)),
            slot(2, Sport::Tennis, start),
            // そのメニューの、期間の外の枠
            slot(3, Sport::Tennis, start - Duration::from_secs(1))
                .with_restored_lesson_menu(Some(menu)),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        book(&bookings, 7, 1).await;
        book(&bookings, 8, 2).await;
        book(&bookings, 9, 3).await;

        let report = GetSlotSupply::new(bookings, no_accounts(), FixedClock(now))
            .execute(GetSlotSupplyQuery {
                from: Some(from),
                to: Some(to),
                lesson_menu_id: Some(menu),
                ..GetSlotSupplyQuery::default()
            })
            .await
            .unwrap();
        assert_eq!(report.dates, Some((from, to)));
        assert_eq!(report.sports[0], SlotSupply::new(Sport::Tennis, 1, 1));
    }
}
