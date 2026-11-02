use std::{cmp::Reverse, collections::BTreeSet};

use domain::{
    identity::AccountId,
    scheduling::{BookingPeriod, BookingStatus, SchedulingError},
};
use time::{Date, Duration};

use super::{
    BookingCoachDisplayNameLookup, BookingList, BookingRepository, StudentDisplayNameLookup,
    calendar_day,
};
use crate::{Unavailable, clock::Clock};

/// 本部が見る予約の一覧のユースケース（admin-web の `listBookings`。本部だけが呼ぶ）。
///
/// 日付（日本時間）の `from` から `to` まで（両端を含む）に開始する予約を読み、開始時刻の古い順に並べる
/// （同じ開始時刻は予約の作成の新しい順、それも同じなら予約 ID の順）。`from` を省けば今日、`to` を省けば `from` から
/// [`BookingPeriod::DEFAULT_DAYS`] 日目。期間の規則（`to` が `from` より前でない・[`BookingPeriod::MAX_DAYS`] 日まで）は
/// domain の [`BookingPeriod`]。`coach_id` があればそのコーチの枠への予約だけに絞る（いないコーチなら空の一覧。
/// 規則は domain の [`Booking::matches_coach`](domain::scheduling::Booking::matches_coach)）。
/// `student_id` があればその生徒が予約した予約だけに絞る（いない生徒なら空の一覧）。
/// `statuses` が空でなければ、そのどれかの状態の予約だけに絞る（空なら状態を問わない）。`trial` があれば、体験予約か
/// （[`Booking::is_trial`](domain::scheduling::Booking::is_trial)）がそれと同じ予約だけに絞る（なければ問わない）。
/// 一覧に出るコーチと生徒の表示名は profile に問い合わせて添える。読むだけで何も変えない。
pub struct ListBookings<B, K, D, C> {
    bookings: B,
    coaches: K,
    students: D,
    clock: C,
}

/// 本部が見る予約の一覧の条件。どれも省ける。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListBookingsQuery {
    /// 期間の始まりの日付（日本時間。含む）
    pub from: Option<Date>,
    /// 期間の終わりの日付（日本時間。含む）
    pub to: Option<Date>,
    /// あればこのコーチの枠への予約だけ
    pub coach_id: Option<AccountId>,
    /// あればこの生徒が予約した予約だけ
    pub student_id: Option<AccountId>,
    /// 空でなければこのどれかの状態の予約だけ（空なら状態を問わない）
    pub statuses: Vec<BookingStatus>,
    /// あれば体験予約か（`true` なら体験予約だけ、`false` なら体験予約でない予約だけ。なければ問わない）
    pub trial: Option<bool>,
}

/// 本部が見る予約の一覧（[`ListBookings`]）と、キャンセルされた予約の一覧（[`super::ListCancelledBookings`]）の失敗。
#[derive(Debug)]
pub enum ListBookingsError {
    /// 期間が規則を満たさない（[`BookingPeriod::new`] の違反）
    Invalid(SchedulingError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<SchedulingError> for ListBookingsError {
    fn from(error: SchedulingError) -> Self {
        Self::Invalid(error)
    }
}

impl From<Unavailable> for ListBookingsError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}

impl<B, K, D, C> ListBookings<B, K, D, C>
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
        query: ListBookingsQuery,
    ) -> Result<BookingList, ListBookingsError> {
        let from = query
            .from
            .unwrap_or_else(|| calendar_day::of(self.clock.now()));
        let to = query.to.unwrap_or_else(|| {
            from.saturating_add(Duration::days(i64::from(BookingPeriod::DEFAULT_DAYS) - 1))
        });
        let period = BookingPeriod::new(calendar_day::start(from), calendar_day::end(to))?;

        let (_, mut bookings) = self
            .bookings
            .slots_starting_between(period.starts_at(), period.ends_at())
            .await?;
        bookings.retain(|booking| {
            period.includes(booking.starts_at())
                && booking.matches_coach(query.coach_id)
                && query
                    .student_id
                    .is_none_or(|student_id| booking.student_id() == student_id)
                && (query.statuses.is_empty() || query.statuses.contains(&booking.status()))
                && query.trial.is_none_or(|trial| booking.is_trial() == trial)
        });
        bookings.sort_by_key(|booking| {
            (
                booking.starts_at(),
                Reverse(booking.created_at()),
                booking.id(),
            )
        });

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
        Ok(BookingList {
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
        scheduling::{Booking, BookingStatus, Slot, SlotId, SlotSeats, SlotStatus},
    };
    use time::Month;

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryBookingRepository, InMemorySlotRepository};

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

    fn slot(id: u128, coach: AccountId, starts_at: SystemTime) -> Slot {
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

    async fn book(
        bookings: &InMemoryBookingRepository,
        student: AccountId,
        slot: u128,
        created_at: SystemTime,
    ) -> Booking {
        reserve(bookings, student, slot, false, created_at).await
    }

    async fn reserve(
        bookings: &InMemoryBookingRepository,
        student: AccountId,
        slot: u128,
        is_trial: bool,
        created_at: SystemTime,
    ) -> Booking {
        bookings
            .reserve(SlotId::from_u128(slot), student, |slot, existing| {
                Booking::reserve(
                    bookings.next_id(),
                    student,
                    is_trial,
                    slot,
                    existing,
                    created_at,
                )
            })
            .await
            .unwrap()
    }

    fn list_bookings(
        bookings: InMemoryBookingRepository,
        lookup: RecordingLookup,
        now: SystemTime,
    ) -> ListBookings<InMemoryBookingRepository, RecordingLookup, RecordingLookup, FixedClock> {
        ListBookings::new(bookings, lookup.clone(), lookup, FixedClock(now))
    }

    fn rows(list: &BookingList) -> Vec<(u128, BookingStatus)> {
        list.bookings
            .iter()
            .map(|booking| (booking.slot_id().as_u128(), booking.status()))
            .collect()
    }

    #[tokio::test]
    async fn lists_bookings_of_every_coach_starting_in_the_days_oldest_first() {
        let slots = InMemorySlotRepository::with_slots([
            // 期間（11 月 2 日〜3 日）の前後の境目
            slot(1, COACH, at(2, 0) - std::time::Duration::from_secs(1)),
            slot(2, COACH, at(2, 0)),
            slot(3, OTHER_COACH, at(3, 10)),
            slot(4, COACH, at(2, 18)),
            slot(5, COACH, at(4, 0)),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        let created = at(1, 0);
        book(&bookings, STUDENT, 1, created).await;
        book(&bookings, STUDENT, 2, created).await;
        let cancelled = book(&bookings, OTHER_STUDENT, 4, created).await;
        bookings
            .update(cancelled.id(), |booking, slot| {
                booking.cancel(OTHER_STUDENT, slot, at(1, 1)).map(|_| ())
            })
            .await
            .unwrap();
        // キャンセルされた枠を取り直した予約は、キャンセル済みより前に出る
        book(&bookings, STUDENT, 4, at(1, 2)).await;
        book(&bookings, OTHER_STUDENT, 3, created).await;
        book(&bookings, STUDENT, 5, created).await;

        let lookup = RecordingLookup::default();
        let list = list_bookings(bookings, lookup.clone(), at(1, 0))
            .execute(ListBookingsQuery {
                from: Some(date(2)),
                to: Some(date(3)),
                coach_id: None,
                student_id: None,
                statuses: Vec::new(),
                trial: None,
            })
            .await
            .unwrap();
        assert_eq!((list.from, list.to), (date(2), date(3)));
        assert_eq!(
            rows(&list),
            [
                (2, BookingStatus::Pending),
                (4, BookingStatus::Pending),
                (4, BookingStatus::Cancelled),
                (3, BookingStatus::Pending),
            ]
        );
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
    async fn without_dates_lists_seven_days_from_today_in_japan() {
        let slots = InMemorySlotRepository::with_slots([
            slot(1, COACH, at(1, 0) - std::time::Duration::from_secs(1)),
            slot(2, COACH, at(1, 0)),
            slot(3, COACH, at(7, 23)),
            slot(4, COACH, at(8, 0)),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        for slot in 1..=4 {
            book(&bookings, STUDENT, slot, at(1, 0)).await;
        }
        // 日本時間の 11 月 1 日 23 時（UTC ではまだ 11 月 1 日 14 時）
        let list = list_bookings(bookings, RecordingLookup::default(), at(1, 23))
            .execute(ListBookingsQuery::default())
            .await
            .unwrap();
        assert_eq!((list.from, list.to), (date(1), date(7)));
        assert_eq!(
            rows(&list),
            [(2, BookingStatus::Pending), (3, BookingStatus::Pending)]
        );
    }

    #[tokio::test]
    async fn without_to_lists_seven_days_from_the_given_day() {
        let list = list_bookings(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            RecordingLookup::default(),
            at(1, 0),
        )
        .execute(ListBookingsQuery {
            from: Some(date(10)),
            to: None,
            coach_id: None,
            student_id: None,
            statuses: Vec::new(),
            trial: None,
        })
        .await
        .unwrap();
        assert_eq!((list.from, list.to), (date(10), date(16)));
    }

    #[tokio::test]
    async fn rejects_days_that_end_before_they_start_or_are_too_long() {
        let lookup = RecordingLookup::default();
        let use_case = list_bookings(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            lookup.clone(),
            at(10, 0),
        );
        for (query, expected) in [
            (
                ListBookingsQuery {
                    from: Some(date(3)),
                    to: Some(date(2)),
                    coach_id: None,
                    student_id: None,
                    statuses: Vec::new(),
                    trial: None,
                },
                SchedulingError::BookingPeriodEndsBeforeItStarts,
            ),
            // 省いた from は今日（11 月 10 日）
            (
                ListBookingsQuery {
                    from: None,
                    to: Some(date(9)),
                    coach_id: None,
                    student_id: None,
                    statuses: Vec::new(),
                    trial: None,
                },
                SchedulingError::BookingPeriodEndsBeforeItStarts,
            ),
            // 11 月 1 日〜12 月 2 日は 32 日
            (
                ListBookingsQuery {
                    from: Some(date(1)),
                    to: Some(Date::from_calendar_date(2026, Month::December, 2).unwrap()),
                    coach_id: None,
                    student_id: None,
                    statuses: Vec::new(),
                    trial: None,
                },
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
                .execute(ListBookingsQuery {
                    from: Some(date(1)),
                    to: Some(Date::from_calendar_date(2026, Month::December, 1).unwrap()),
                    coach_id: None,
                    student_id: None,
                    statuses: Vec::new(),
                    trial: None,
                })
                .await
                .is_ok()
        );
        assert!(lookup.coaches_asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn with_coach_lists_only_bookings_of_that_coach_in_the_days() {
        let slots = InMemorySlotRepository::with_slots([
            slot(1, COACH, at(2, 10)),
            slot(2, OTHER_COACH, at(2, 9)),
            slot(3, OTHER_COACH, at(3, 18)),
            // 期間（11 月 2 日〜3 日）の後
            slot(4, OTHER_COACH, at(4, 0)),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        for (student, slot) in [(STUDENT, 1), (OTHER_STUDENT, 2), (STUDENT, 3), (STUDENT, 4)] {
            book(&bookings, student, slot, at(1, 0)).await;
        }

        let lookup = RecordingLookup::default();
        let list = list_bookings(bookings, lookup.clone(), at(1, 0))
            .execute(ListBookingsQuery {
                from: Some(date(2)),
                to: Some(date(3)),
                coach_id: Some(OTHER_COACH),
                student_id: None,
                statuses: Vec::new(),
                trial: None,
            })
            .await
            .unwrap();
        assert_eq!((list.from, list.to), (date(2), date(3)));
        assert_eq!(
            rows(&list),
            [(2, BookingStatus::Pending), (3, BookingStatus::Pending)]
        );
        assert_eq!(
            *lookup.coaches_asked.lock().unwrap(),
            [BTreeSet::from([OTHER_COACH])],
            "絞った後の予約のコーチだけを問い合わせる"
        );
        assert_eq!(
            *lookup.students_asked.lock().unwrap(),
            [BTreeSet::from([STUDENT, OTHER_STUDENT])]
        );
    }

    #[tokio::test]
    async fn with_statuses_lists_only_bookings_in_those_statuses() {
        let slots = InMemorySlotRepository::with_slots([
            slot(1, COACH, at(2, 9)),
            slot(2, COACH, at(2, 10)),
            slot(3, OTHER_COACH, at(2, 11)),
            slot(4, OTHER_COACH, at(2, 12)),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        let mut booked = Vec::new();
        for slot in 1..=4 {
            booked.push(book(&bookings, STUDENT, slot, at(1, 0)).await.id());
        }
        let id_of = |slot: usize| booked[slot - 1];
        bookings
            .update(id_of(2), |booking, slot| {
                booking.confirm(STUDENT, slot, at(1, 1))
            })
            .await
            .unwrap();
        bookings
            .update(id_of(3), |booking, slot| {
                booking.cancel(STUDENT, slot, at(1, 1)).map(|_| ())
            })
            .await
            .unwrap();
        // キャンセルされた枠を他の生徒が取り直した（確定待ち）
        book(&bookings, OTHER_STUDENT, 3, at(1, 2)).await;
        bookings
            .update(id_of(4), |booking, slot| {
                booking.confirm(STUDENT, slot, at(1, 1))
            })
            .await
            .unwrap();
        let use_case = list_bookings(bookings, RecordingLookup::default(), at(1, 0));

        for (statuses, expected) in [
            (
                vec![BookingStatus::Pending],
                vec![(1, BookingStatus::Pending), (3, BookingStatus::Pending)],
            ),
            (
                vec![BookingStatus::Confirmed],
                vec![(2, BookingStatus::Confirmed), (4, BookingStatus::Confirmed)],
            ),
            (
                vec![BookingStatus::Cancelled],
                vec![(3, BookingStatus::Cancelled)],
            ),
            // 複数なら、どれかの状態のもの。並べ方は変わらない
            (
                vec![BookingStatus::Cancelled, BookingStatus::Pending],
                vec![
                    (1, BookingStatus::Pending),
                    (3, BookingStatus::Pending),
                    (3, BookingStatus::Cancelled),
                ],
            ),
            // 空なら状態を問わない（今までと同じ）
            (
                vec![],
                vec![
                    (1, BookingStatus::Pending),
                    (2, BookingStatus::Confirmed),
                    (3, BookingStatus::Pending),
                    (3, BookingStatus::Cancelled),
                    (4, BookingStatus::Confirmed),
                ],
            ),
        ] {
            let list = use_case
                .execute(ListBookingsQuery {
                    from: Some(date(2)),
                    to: Some(date(2)),
                    coach_id: None,
                    student_id: None,
                    statuses: statuses.clone(),
                    trial: None,
                })
                .await
                .unwrap();
            assert_eq!(rows(&list), expected, "{statuses:?}");
        }

        // コーチとも組み合わせられる
        let list = use_case
            .execute(ListBookingsQuery {
                from: Some(date(2)),
                to: Some(date(2)),
                coach_id: Some(OTHER_COACH),
                student_id: None,
                statuses: vec![BookingStatus::Pending],
                trial: None,
            })
            .await
            .unwrap();
        assert_eq!(rows(&list), [(3, BookingStatus::Pending)]);
    }

    #[tokio::test]
    async fn with_statuses_matching_nothing_is_an_empty_list_without_asking_for_display_names() {
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([slot(
            1,
            COACH,
            at(2, 10),
        )]));
        book(&bookings, STUDENT, 1, at(1, 0)).await;
        let lookup = RecordingLookup::default();
        let list = list_bookings(bookings, lookup.clone(), at(1, 0))
            .execute(ListBookingsQuery {
                statuses: vec![BookingStatus::Confirmed, BookingStatus::Cancelled],
                ..ListBookingsQuery::default()
            })
            .await
            .unwrap();
        assert!(list.bookings.is_empty());
        assert!(lookup.coaches_asked.lock().unwrap().is_empty());
        assert!(lookup.students_asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn with_trial_lists_only_trial_or_only_regular_bookings() {
        let slots = InMemorySlotRepository::with_slots([
            slot(1, COACH, at(2, 9)),
            slot(2, COACH, at(2, 10)),
            slot(3, OTHER_COACH, at(2, 11)),
            slot(4, OTHER_COACH, at(2, 12)),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        let trial = reserve(&bookings, STUDENT, 1, true, at(1, 0)).await;
        book(&bookings, STUDENT, 2, at(1, 0)).await;
        reserve(&bookings, OTHER_STUDENT, 3, true, at(1, 0)).await;
        book(&bookings, OTHER_STUDENT, 4, at(1, 0)).await;
        // キャンセルした体験予約も体験予約のまま
        bookings
            .update(trial.id(), |booking, slot| {
                booking.cancel(STUDENT, slot, at(1, 1)).map(|_| ())
            })
            .await
            .unwrap();
        let use_case = list_bookings(bookings, RecordingLookup::default(), at(1, 0));

        for (trial, expected) in [
            (
                Some(true),
                vec![(1, BookingStatus::Cancelled), (3, BookingStatus::Pending)],
            ),
            (
                Some(false),
                vec![(2, BookingStatus::Pending), (4, BookingStatus::Pending)],
            ),
            // なければ体験予約かを問わない（今までと同じ）
            (
                None,
                vec![
                    (1, BookingStatus::Cancelled),
                    (2, BookingStatus::Pending),
                    (3, BookingStatus::Pending),
                    (4, BookingStatus::Pending),
                ],
            ),
        ] {
            let list = use_case
                .execute(ListBookingsQuery {
                    from: Some(date(2)),
                    to: Some(date(2)),
                    trial,
                    ..ListBookingsQuery::default()
                })
                .await
                .unwrap();
            assert_eq!(rows(&list), expected, "{trial:?}");
            assert!(
                list.bookings
                    .iter()
                    .all(|booking| trial.is_none_or(|trial| booking.is_trial() == trial))
            );
        }

        // コーチ・状態とも組み合わせられる
        let list = use_case
            .execute(ListBookingsQuery {
                from: Some(date(2)),
                to: Some(date(2)),
                coach_id: Some(OTHER_COACH),
                student_id: None,
                statuses: vec![BookingStatus::Pending],
                trial: Some(true),
            })
            .await
            .unwrap();
        assert_eq!(rows(&list), [(3, BookingStatus::Pending)]);
        let list = use_case
            .execute(ListBookingsQuery {
                from: Some(date(2)),
                to: Some(date(2)),
                coach_id: Some(COACH),
                student_id: None,
                statuses: vec![BookingStatus::Pending],
                trial: Some(true),
            })
            .await
            .unwrap();
        assert!(list.bookings.is_empty());
    }

    #[tokio::test]
    async fn with_student_lists_only_bookings_of_that_student_in_the_days() {
        let slots = InMemorySlotRepository::with_slots([
            slot(1, COACH, at(2, 10)),
            slot(2, OTHER_COACH, at(2, 9)),
            slot(3, OTHER_COACH, at(3, 18)),
            slot(4, COACH, at(3, 12)),
            // 期間（11 月 2 日〜3 日）の後
            slot(5, OTHER_COACH, at(4, 0)),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        let trial = reserve(&bookings, OTHER_STUDENT, 2, true, at(1, 0)).await;
        for (student, slot) in [
            (STUDENT, 1),
            (OTHER_STUDENT, 3),
            (STUDENT, 4),
            (OTHER_STUDENT, 5),
        ] {
            book(&bookings, student, slot, at(1, 0)).await;
        }
        bookings
            .update(trial.id(), |booking, slot| {
                booking.cancel(OTHER_STUDENT, slot, at(1, 1)).map(|_| ())
            })
            .await
            .unwrap();

        let lookup = RecordingLookup::default();
        let use_case = list_bookings(bookings, lookup.clone(), at(1, 0));
        let list = use_case
            .execute(ListBookingsQuery {
                from: Some(date(2)),
                to: Some(date(3)),
                student_id: Some(OTHER_STUDENT),
                ..ListBookingsQuery::default()
            })
            .await
            .unwrap();
        assert_eq!((list.from, list.to), (date(2), date(3)));
        assert_eq!(
            rows(&list),
            [(2, BookingStatus::Cancelled), (3, BookingStatus::Pending)]
        );
        assert!(
            list.bookings
                .iter()
                .all(|booking| booking.student_id() == OTHER_STUDENT)
        );
        assert_eq!(
            *lookup.students_asked.lock().unwrap(),
            [BTreeSet::from([OTHER_STUDENT])],
            "絞った後の予約の生徒だけを問い合わせる"
        );
        assert_eq!(
            *lookup.coaches_asked.lock().unwrap(),
            [BTreeSet::from([OTHER_COACH])]
        );

        // コーチ・状態・体験予約かとも組み合わせられる
        for (query, expected) in [
            (
                ListBookingsQuery {
                    coach_id: Some(COACH),
                    student_id: Some(STUDENT),
                    ..ListBookingsQuery::default()
                },
                vec![(1, BookingStatus::Pending), (4, BookingStatus::Pending)],
            ),
            (
                ListBookingsQuery {
                    coach_id: Some(COACH),
                    student_id: Some(OTHER_STUDENT),
                    ..ListBookingsQuery::default()
                },
                vec![],
            ),
            (
                ListBookingsQuery {
                    student_id: Some(OTHER_STUDENT),
                    statuses: vec![BookingStatus::Pending],
                    ..ListBookingsQuery::default()
                },
                vec![(3, BookingStatus::Pending)],
            ),
            (
                ListBookingsQuery {
                    student_id: Some(OTHER_STUDENT),
                    trial: Some(true),
                    ..ListBookingsQuery::default()
                },
                vec![(2, BookingStatus::Cancelled)],
            ),
        ] {
            let list = use_case
                .execute(ListBookingsQuery {
                    from: Some(date(2)),
                    to: Some(date(3)),
                    ..query.clone()
                })
                .await
                .unwrap();
            assert_eq!(rows(&list), expected, "{query:?}");
        }
    }

    #[tokio::test]
    async fn with_unknown_student_is_an_empty_list() {
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([slot(
            1,
            COACH,
            at(2, 10),
        )]));
        book(&bookings, STUDENT, 1, at(1, 0)).await;
        let lookup = RecordingLookup::default();
        let use_case = list_bookings(bookings, lookup.clone(), at(1, 0));
        // 生徒でない ID（コーチ）・ない ID
        for student_id in [COACH, AccountId::from_u128(99)] {
            let list = use_case
                .execute(ListBookingsQuery {
                    student_id: Some(student_id),
                    ..ListBookingsQuery::default()
                })
                .await
                .unwrap();
            assert_eq!((list.from, list.to), (date(1), date(7)));
            assert!(list.bookings.is_empty(), "{student_id:?}");
        }
        assert!(lookup.students_asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn with_unknown_coach_is_an_empty_list() {
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([slot(
            1,
            COACH,
            at(2, 10),
        )]));
        book(&bookings, STUDENT, 1, at(1, 0)).await;
        let lookup = RecordingLookup::default();
        let list = list_bookings(bookings, lookup.clone(), at(1, 0))
            .execute(ListBookingsQuery {
                coach_id: Some(AccountId::from_u128(99)),
                ..ListBookingsQuery::default()
            })
            .await
            .unwrap();
        assert_eq!((list.from, list.to), (date(1), date(7)));
        assert!(list.bookings.is_empty());
        assert!(lookup.coaches_asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn no_bookings_is_an_empty_list_without_asking_for_display_names() {
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([slot(
            1,
            COACH,
            at(2, 10),
        )]));
        let lookup = RecordingLookup::default();
        let list = list_bookings(bookings, lookup.clone(), at(1, 0))
            .execute(ListBookingsQuery::default())
            .await
            .unwrap();
        assert!(list.bookings.is_empty());
        assert!(list.coaches.is_empty() && list.students.is_empty());
        assert!(lookup.coaches_asked.lock().unwrap().is_empty());
        assert!(lookup.students_asked.lock().unwrap().is_empty());
    }
}
