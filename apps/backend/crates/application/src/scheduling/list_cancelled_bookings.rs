use std::{cmp::Reverse, collections::BTreeSet};

use domain::{identity::AccountId, scheduling::BookingPeriod};
use time::{Date, Duration};

use super::{
    BookingCoachDisplayNameLookup, BookingRepository, CancelledBookingList, ListBookingsError,
    StudentDisplayNameLookup, calendar_day,
};
use crate::clock::Clock;

/// 本部が見るキャンセルされた予約の一覧のユースケース（admin-web の `listCancelledBookings`。本部だけが呼ぶ）。
///
/// 日付（日本時間）の `from` から `to` まで（両端を含む）にキャンセルした予約を読み、キャンセルした時刻の新しい順に並べる
/// （同じ時刻は予約 ID の順）。日付を両方省けば今日までの [`BookingPeriod::DEFAULT_DAYS`] 日、`from` だけなら `from` から、
/// `to` だけなら `to` までの [`BookingPeriod::DEFAULT_DAYS`] 日。期間の規則（`to` が `from` より前でない・
/// [`BookingPeriod::MAX_DAYS`] 日まで）は domain の [`BookingPeriod`]。`coach_id` があればそのコーチの枠への予約だけに
/// 絞る（いないコーチなら空の一覧。規則は [`super::ListBookings`] と同じ domain の
/// [`Booking::matches_coach`](domain::scheduling::Booking::matches_coach)）。キャンセルした人は domain の
/// [`Booking::cancelled_by`](domain::scheduling::Booking::cancelled_by)。一覧に出るコーチと生徒の表示名は profile に
/// 問い合わせて添える。読むだけで何も変えない。
pub struct ListCancelledBookings<B, K, D, C> {
    bookings: B,
    coaches: K,
    students: D,
    clock: C,
}

/// 本部が見るキャンセルされた予約の一覧の条件。どれも省ける。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListCancelledBookingsQuery {
    /// 期間の始まりの日付（日本時間。含む）
    pub from: Option<Date>,
    /// 期間の終わりの日付（日本時間。含む）
    pub to: Option<Date>,
    /// あればこのコーチの枠への予約だけ
    pub coach_id: Option<AccountId>,
}

impl<B, K, D, C> ListCancelledBookings<B, K, D, C>
where
    B: BookingRepository,
    K: BookingCoachDisplayNameLookup,
    D: StudentDisplayNameLookup,
    C: Clock,
{
    pub fn new(bookings: B, coaches: K, students: D, clock: C) -> Self {
        Self {
            bookings,
            coaches,
            students,
            clock,
        }
    }

    pub async fn execute(
        &self,
        query: ListCancelledBookingsQuery,
    ) -> Result<CancelledBookingList, ListBookingsError> {
        let span = Duration::days(i64::from(BookingPeriod::DEFAULT_DAYS) - 1);
        let (from, to) = match (query.from, query.to) {
            (Some(from), Some(to)) => (from, to),
            (Some(from), None) => (from, from.saturating_add(span)),
            (None, Some(to)) => (to.saturating_sub(span), to),
            (None, None) => {
                let today = calendar_day::of(self.clock.now());
                (today.saturating_sub(span), today)
            }
        };
        let period = BookingPeriod::new(calendar_day::start(from), calendar_day::end(to))?;

        let mut bookings = self
            .bookings
            .bookings_cancelled_between(period.starts_at(), period.ends_at())
            .await?;
        bookings.retain(|booking| {
            booking.cancelled_at().is_some_and(|at| period.includes(at))
                && booking.matches_coach(query.coach_id)
        });
        bookings.sort_by_key(|booking| (Reverse(booking.cancelled_at()), booking.id()));

        let coach_ids: BTreeSet<AccountId> = bookings.iter().map(|b| b.coach_id()).collect();
        let student_ids: BTreeSet<AccountId> = bookings.iter().map(|b| b.student_id()).collect();
        let (coaches, students) = if bookings.is_empty() {
            Default::default()
        } else {
            (
                self.coaches.find_coach_display_names(&coach_ids).await?,
                self.students
                    .find_student_display_names(&student_ids)
                    .await?,
            )
        };
        Ok(CancelledBookingList {
            from,
            to,
            bookings,
            coaches,
            students,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
        time::SystemTime,
    };

    use domain::{
        catalog::Sport,
        profile::DisplayName,
        scheduling::{
            Booking, BookingCanceller, BookingId, SchedulingError, Slot, SlotId, SlotSeats,
            SlotStatus,
        },
    };
    use time::Month;

    use super::*;
    use crate::{
        Unavailable,
        in_memory::{FixedClock, InMemoryBookingRepository, InMemorySlotRepository},
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn date(day: u8) -> Date {
        Date::from_calendar_date(2026, Month::November, day).unwrap()
    }

    /// 2026 年 11 月 `day` 日の日本時間 `hour` 時
    fn at(day: u8, hour: u8) -> SystemTime {
        calendar_day::start(date(day)) + std::time::Duration::from_secs(u64::from(hour) * 3600)
    }

    fn slot(id: u128, coach: AccountId) -> Slot {
        // 枠の開始はキャンセルした時刻と関係しない（11 月 20 日に揃える）
        let starts_at = at(20, 0) + std::time::Duration::from_secs(id as u64 * 3600);
        Slot::restore(
            SlotId::from_u128(id),
            coach,
            Sport::Tennis,
            starts_at,
            starts_at + std::time::Duration::from_secs(3600),
            SlotStatus::Open,
            SlotSeats::default(),
        )
        .unwrap()
    }

    fn name(value: &str) -> DisplayName {
        DisplayName::new(value).unwrap()
    }

    /// `COACH` と `STUDENT` の表示名だけを知っていて、問い合わせた ID を記録する。
    #[derive(Clone, Default)]
    struct RecordingLookup {
        coaches_asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
        students_asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
    }

    impl BookingCoachDisplayNameLookup for RecordingLookup {
        async fn find_coach_display_names(
            &self,
            coach_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            self.coaches_asked.lock().unwrap().push(coach_ids.clone());
            Ok(coach_ids
                .iter()
                .filter(|id| **id == COACH)
                .map(|id| (*id, name("たなか")))
                .collect())
        }
    }

    impl StudentDisplayNameLookup for RecordingLookup {
        async fn find_student_display_names(
            &self,
            student_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            self.students_asked
                .lock()
                .unwrap()
                .push(student_ids.clone());
            Ok(student_ids
                .iter()
                .filter(|id| **id == STUDENT)
                .map(|id| (*id, name("あおき")))
                .collect())
        }
    }

    async fn book(bookings: &InMemoryBookingRepository, student: AccountId, slot: u128) -> Booking {
        bookings
            .reserve(SlotId::from_u128(slot), student, |slot, existing| {
                Booking::reserve(
                    BookingId::from_u128(slot.id().as_u128() * 100 + existing.len() as u128),
                    student,
                    false,
                    slot,
                    existing,
                    at(1, 0),
                )
            })
            .await
            .unwrap()
    }

    async fn cancel(bookings: &InMemoryBookingRepository, booking: &Booking, at: SystemTime) {
        let student = booking.student_id();
        bookings
            .update(booking.id(), |booking, slot| {
                booking.cancel(student, slot, at).map(|_| ())
            })
            .await
            .unwrap();
    }

    fn list_cancelled_bookings(
        bookings: InMemoryBookingRepository,
        lookup: RecordingLookup,
        now: SystemTime,
    ) -> ListCancelledBookings<
        InMemoryBookingRepository,
        RecordingLookup,
        RecordingLookup,
        FixedClock,
    > {
        ListCancelledBookings::new(bookings, lookup.clone(), lookup, FixedClock(now))
    }

    fn query(from: Option<Date>, to: Option<Date>) -> ListCancelledBookingsQuery {
        ListCancelledBookingsQuery {
            from,
            to,
            coach_id: None,
        }
    }

    /// 一覧の予約の枠の ID
    fn rows(list: &CancelledBookingList) -> Vec<u128> {
        list.bookings
            .iter()
            .map(|booking| booking.slot_id().as_u128())
            .collect()
    }

    #[tokio::test]
    async fn lists_bookings_cancelled_in_the_days_newest_cancellation_first() {
        let slots = InMemorySlotRepository::with_slots(
            [
                (1, COACH),
                (2, COACH),
                (3, OTHER_COACH),
                (4, COACH),
                (5, COACH),
                (6, OTHER_COACH),
            ]
            .map(|(id, coach)| slot(id, coach)),
        );
        let bookings = InMemoryBookingRepository::new(slots);
        let made = [
            book(&bookings, STUDENT, 1).await,
            book(&bookings, STUDENT, 2).await,
            book(&bookings, OTHER_STUDENT, 3).await,
            book(&bookings, STUDENT, 4).await,
            book(&bookings, STUDENT, 5).await,
            // キャンセルしていない予約は出ない
            book(&bookings, STUDENT, 6).await,
        ];
        // 期間（11 月 2 日〜3 日）の前後の境目
        cancel(
            &bookings,
            &made[0],
            at(2, 0) - std::time::Duration::from_nanos(1),
        )
        .await;
        cancel(&bookings, &made[1], at(2, 0)).await;
        cancel(&bookings, &made[2], at(3, 23)).await;
        cancel(&bookings, &made[3], at(2, 12)).await;
        cancel(&bookings, &made[4], at(4, 0)).await;
        // キャンセルした枠を取り直した予約（確定待ち）は出ない
        book(&bookings, OTHER_STUDENT, 4).await;

        let lookup = RecordingLookup::default();
        let list = list_cancelled_bookings(bookings, lookup.clone(), at(10, 0))
            .execute(query(Some(date(2)), Some(date(3))))
            .await
            .unwrap();
        assert_eq!((list.from, list.to), (date(2), date(3)));
        assert_eq!(rows(&list), [3, 4, 2]);
        assert!(
            list.bookings
                .iter()
                .all(|booking| booking.cancelled_by() == Some(BookingCanceller::Student))
        );
        assert_eq!(list.bookings[0].cancelled_at(), Some(at(3, 23)));
        assert_eq!(list.coaches, BTreeMap::from([(COACH, name("たなか"))]));
        assert_eq!(list.students, BTreeMap::from([(STUDENT, name("あおき"))]));
        assert_eq!(
            *lookup.coaches_asked.lock().unwrap(),
            [BTreeSet::from([COACH, OTHER_COACH])],
            "一覧のコーチを重ねずに 1 回で問い合わせる"
        );
        assert_eq!(
            *lookup.students_asked.lock().unwrap(),
            [BTreeSet::from([STUDENT, OTHER_STUDENT])]
        );
    }

    #[tokio::test]
    async fn coach_id_narrows_to_the_coach_keeping_the_days_and_the_order() {
        let slots = InMemorySlotRepository::with_slots(
            [(1, COACH), (2, OTHER_COACH), (3, COACH), (4, COACH)]
                .map(|(id, coach)| slot(id, coach)),
        );
        let bookings = InMemoryBookingRepository::new(slots);
        for (slot, cancelled_at) in [
            (1, at(2, 9)),
            (2, at(2, 10)),
            (3, at(3, 8)),
            // 期間（11 月 2 日〜3 日）の外
            (4, at(4, 0)),
        ] {
            let booking = book(&bookings, STUDENT, slot).await;
            cancel(&bookings, &booking, cancelled_at).await;
        }

        let lookup = RecordingLookup::default();
        let use_case = list_cancelled_bookings(bookings, lookup.clone(), at(10, 0));
        let with_coach = |coach_id| ListCancelledBookingsQuery {
            coach_id: Some(coach_id),
            ..query(Some(date(2)), Some(date(3)))
        };
        let list = use_case.execute(with_coach(COACH)).await.unwrap();
        assert_eq!((list.from, list.to), (date(2), date(3)));
        assert_eq!(rows(&list), [3, 1]);
        assert_eq!(
            *lookup.coaches_asked.lock().unwrap(),
            [BTreeSet::from([COACH])],
            "絞ったあとのコーチだけを問い合わせる"
        );

        let list = use_case.execute(with_coach(OTHER_COACH)).await.unwrap();
        assert_eq!(rows(&list), [2]);

        // ない ID・コーチでない ID（生徒）は空の一覧で、表示名を問い合わせない
        for coach_id in [AccountId::from_u128(99), STUDENT] {
            let list = use_case.execute(with_coach(coach_id)).await.unwrap();
            assert!(list.bookings.is_empty(), "{coach_id:?}");
        }
        assert_eq!(lookup.coaches_asked.lock().unwrap().len(), 2);

        // 期間の規則はコーチで絞っても同じ
        let error = use_case
            .execute(ListCancelledBookingsQuery {
                coach_id: Some(COACH),
                ..query(Some(date(3)), Some(date(2)))
            })
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            ListBookingsError::Invalid(SchedulingError::BookingPeriodEndsBeforeItStarts)
        ));
    }

    #[tokio::test]
    async fn same_cancellation_time_is_in_booking_id_order() {
        let slots = InMemorySlotRepository::with_slots([slot(2, COACH), slot(1, COACH)]);
        let bookings = InMemoryBookingRepository::new(slots);
        let second = book(&bookings, STUDENT, 2).await;
        let first = book(&bookings, STUDENT, 1).await;
        cancel(&bookings, &second, at(2, 9)).await;
        cancel(&bookings, &first, at(2, 9)).await;

        let list = list_cancelled_bookings(bookings, RecordingLookup::default(), at(2, 10))
            .execute(ListCancelledBookingsQuery::default())
            .await
            .unwrap();
        let ids: Vec<BookingId> = list.bookings.iter().map(Booking::id).collect();
        assert_eq!(ids, [first.id(), second.id()]);
    }

    #[tokio::test]
    async fn without_dates_lists_seven_days_up_to_today_in_japan() {
        let slots = InMemorySlotRepository::with_slots((1..=4).map(|id| slot(id, COACH)));
        let bookings = InMemoryBookingRepository::new(slots);
        for (slot, cancelled_at) in [
            (1, at(1, 0) - std::time::Duration::from_nanos(1)),
            (2, at(1, 0)),
            (3, at(7, 23)),
            (4, at(8, 0)),
        ] {
            let booking = book(&bookings, STUDENT, slot).await;
            cancel(&bookings, &booking, cancelled_at).await;
        }
        // 日本時間の 11 月 7 日 23 時（UTC ではまだ 11 月 7 日 14 時）
        let use_case = list_cancelled_bookings(bookings, RecordingLookup::default(), at(7, 23));
        let list = use_case
            .execute(ListCancelledBookingsQuery::default())
            .await
            .unwrap();
        assert_eq!((list.from, list.to), (date(1), date(7)));
        assert_eq!(rows(&list), [3, 2]);

        // from だけなら from から 7 日、to だけなら to までの 7 日
        let list = use_case.execute(query(Some(date(2)), None)).await.unwrap();
        assert_eq!((list.from, list.to), (date(2), date(8)));
        assert_eq!(rows(&list), [4, 3]);
        let list = use_case.execute(query(None, Some(date(8)))).await.unwrap();
        assert_eq!((list.from, list.to), (date(2), date(8)));
    }

    #[tokio::test]
    async fn rejects_days_that_end_before_they_start_or_are_too_long() {
        let lookup = RecordingLookup::default();
        let use_case = list_cancelled_bookings(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            lookup.clone(),
            at(10, 0),
        );
        let december = |day| Date::from_calendar_date(2026, Month::December, day).unwrap();
        for (query, expected) in [
            (
                query(Some(date(3)), Some(date(2))),
                SchedulingError::BookingPeriodEndsBeforeItStarts,
            ),
            // 11 月 1 日〜12 月 2 日は 32 日
            (
                query(Some(date(1)), Some(december(2))),
                SchedulingError::BookingPeriodTooLong { max_days: 31 },
            ),
        ] {
            let error = use_case.execute(query.clone()).await.unwrap_err();
            assert!(
                matches!(&error, ListBookingsError::Invalid(e) if *e == expected),
                "{query:?}: {error:?}"
            );
        }
        // 11 月 1 日〜12 月 1 日の 31 日は読める
        assert!(
            use_case
                .execute(query(Some(date(1)), Some(december(1))))
                .await
                .is_ok()
        );
        assert!(lookup.coaches_asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn no_cancellations_is_an_empty_list_without_asking_for_display_names() {
        let bookings =
            InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([slot(1, COACH)]));
        book(&bookings, STUDENT, 1).await;
        let lookup = RecordingLookup::default();
        let list = list_cancelled_bookings(bookings, lookup.clone(), at(2, 0))
            .execute(ListCancelledBookingsQuery::default())
            .await
            .unwrap();
        assert!(list.bookings.is_empty());
        assert!(list.coaches.is_empty() && list.students.is_empty());
        assert!(lookup.coaches_asked.lock().unwrap().is_empty());
        assert!(lookup.students_asked.lock().unwrap().is_empty());
    }
}
