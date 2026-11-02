use std::{cmp::Reverse, collections::BTreeSet};

use domain::identity::AccountId;
use time::Date;

use super::{
    BookingDates, BookingRepository, CoachBookingList, ListBookingsError, StudentDisplayNameLookup,
    calendar_day,
};
use crate::clock::Clock;

/// コーチの自分の予約の一覧のユースケース（admin-web の `listMyCoachBookings`。コーチだけが呼ぶ）。
///
/// コーチ `coach_id` のスロットへの予約を、キャンセル済みも含めて予約されたスロットと一緒に読み、スロットの開始時刻の
/// 古い順に並べる（同じ開始時刻は予約の作成の新しい順、それも同じなら予約 ID の順）。日付の `from`・`to` をどちらも
/// 省けば期間で絞らない。どちらかがあれば、本部が見る予約の一覧（[`super::ListBookings`]）と同じ規則（[`BookingDates`]）で
/// 期間にし、スロットの開始がその期間に入る予約だけに絞る。一覧に出る生徒の表示名は profile に問い合わせて
/// （[`StudentDisplayNameLookup`]）添える。読むだけで何も変えない。
pub struct ListCoachBookings<B, D, C> {
    bookings: B,
    students: D,
    clock: C,
}

/// コーチの自分の予約の一覧の条件。どれも省ける。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListCoachBookingsQuery {
    /// 期間の始まりの日付（日本時間。含む）
    pub from: Option<Date>,
    /// 期間の終わりの日付（日本時間。含む）
    pub to: Option<Date>,
}

impl<B, D, C> ListCoachBookings<B, D, C>
where
    B: BookingRepository,
    D: StudentDisplayNameLookup,
    C: Clock,
{
    pub fn new(bookings: B, students: D, clock: C) -> Self {
        Self {
            bookings,
            students,
            clock,
        }
    }

    pub async fn execute(
        &self,
        coach_id: AccountId,
        query: ListCoachBookingsQuery,
    ) -> Result<CoachBookingList, ListBookingsError> {
        let dates = if query.from.is_none() && query.to.is_none() {
            None
        } else {
            Some(BookingDates::resolve(
                query.from,
                query.to,
                calendar_day::of(self.clock.now()),
            )?)
        };
        let mut items = match dates {
            None => self.bookings.bookings_of_coach(coach_id).await?,
            Some(dates) => {
                let period = dates.period();
                let mut items = self
                    .bookings
                    .bookings_of_coach_starting_between(
                        coach_id,
                        period.starts_at(),
                        period.ends_at(),
                    )
                    .await?;
                items.retain(|(_, slot)| period.includes(slot.starts_at()));
                items
            }
        };
        items.sort_by_key(|(booking, slot)| {
            (
                slot.starts_at(),
                Reverse(booking.created_at()),
                booking.id(),
            )
        });
        let student_ids: BTreeSet<AccountId> = items
            .iter()
            .map(|(booking, _)| booking.student_id())
            .collect();
        let students = if student_ids.is_empty() {
            Default::default()
        } else {
            self.students
                .find_student_display_names(&student_ids)
                .await?
        };
        Ok(CoachBookingList {
            from: dates.map(|dates| dates.from()),
            to: dates.map(|dates| dates.to()),
            items,
            students,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
        time::{Duration, SystemTime},
    };

    use domain::{
        catalog::Sport,
        profile::DisplayName,
        scheduling::{Booking, BookingStatus, SchedulingError, Slot, SlotId, SlotStatus},
    };
    use time::Month;

    use super::*;
    use crate::{
        Unavailable,
        in_memory::{FixedClock, InMemoryBookingRepository, InMemorySlotRepository},
        scheduling::BookingRepository,
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(hour * 3600)
    }

    fn date(day: u8) -> Date {
        Date::from_calendar_date(2026, Month::November, day).unwrap()
    }

    /// 2026 年 11 月 `day` 日の日本時間 0 時から `hour` 時間後（の UNIX 紀元からの時間数）
    fn hour_of(day: u8, hour: u64) -> u64 {
        calendar_day::start(date(day))
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            / 3600
            + hour
    }

    /// 今は 2026 年 11 月 2 日の日本時間 12 時
    fn clock() -> FixedClock {
        FixedClock(at(hour_of(2, 12)))
    }

    fn slot(id: u128, coach: AccountId, hour: u64) -> Slot {
        Slot::restore(
            SlotId::from_u128(id),
            coach,
            Sport::Tennis,
            at(hour),
            at(hour + 1),
            SlotStatus::Open,
            domain::scheduling::SlotSeats::default(),
        )
        .unwrap()
    }

    fn name() -> DisplayName {
        DisplayName::new("あおき").unwrap()
    }

    /// `STUDENT` の表示名だけを知っていて（`OTHER_STUDENT` は引けない）、問い合わせた ID を記録する。
    #[derive(Clone, Default)]
    struct RecordingLookup {
        asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
    }

    impl StudentDisplayNameLookup for RecordingLookup {
        async fn find_student_display_names(
            &self,
            student_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            self.asked.lock().unwrap().push(student_ids.clone());
            Ok(student_ids
                .iter()
                .filter(|id| **id == STUDENT)
                .map(|id| (*id, name()))
                .collect())
        }
    }

    async fn book(
        bookings: &InMemoryBookingRepository,
        student: AccountId,
        slot: u128,
        created_hour: u64,
    ) -> Booking {
        bookings
            .reserve(SlotId::from_u128(slot), student, |slot, existing| {
                Booking::reserve(
                    bookings.next_id(),
                    student,
                    false,
                    slot,
                    existing,
                    at(created_hour),
                )
            })
            .await
            .unwrap()
    }

    fn ids(list: &CoachBookingList) -> Vec<(u128, BookingStatus)> {
        list.items
            .iter()
            .map(|(booking, slot)| (slot.id().as_u128(), booking.status()))
            .collect()
    }

    #[tokio::test]
    async fn lists_bookings_of_own_slots_including_cancelled_ones_oldest_slot_first() {
        let slots = InMemorySlotRepository::with_slots([
            slot(1, COACH, 40),
            slot(2, COACH, 20),
            slot(3, COACH, 30),
            slot(4, OTHER_COACH, 10),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        book(&bookings, STUDENT, 1, 0).await;
        let cancelled = book(&bookings, OTHER_STUDENT, 3, 1).await;
        bookings
            .update(cancelled.id(), |booking, slot| {
                booking.cancel(OTHER_STUDENT, slot, at(2)).map(|_| ())
            })
            .await
            .unwrap();
        // キャンセルされた枠を取り直した予約は、キャンセル済みより前に出る
        book(&bookings, STUDENT, 3, 3).await;
        book(&bookings, OTHER_STUDENT, 2, 4).await;
        // 別のコーチの枠への予約は出ない
        book(&bookings, STUDENT, 4, 5).await;

        let lookup = RecordingLookup::default();
        let list = ListCoachBookings::new(bookings, lookup.clone(), clock())
            .execute(COACH, ListCoachBookingsQuery::default())
            .await
            .unwrap();
        assert_eq!(
            ids(&list),
            [
                (2, BookingStatus::Pending),
                (3, BookingStatus::Pending),
                (3, BookingStatus::Cancelled),
                (1, BookingStatus::Pending),
            ]
        );
        assert!(
            list.items
                .iter()
                .all(|(booking, slot)| booking.coach_id() == COACH
                    && slot.coach_id() == COACH
                    && booking.slot_id() == slot.id())
        );
        assert_eq!(
            list.students,
            BTreeMap::from([(STUDENT, name())]),
            "表示名が引けない生徒（OTHER_STUDENT）は含まない"
        );
        assert_eq!(
            *lookup.asked.lock().unwrap(),
            [BTreeSet::from([STUDENT, OTHER_STUDENT])],
            "一覧の生徒を重ねずに 1 回で問い合わせる"
        );
    }

    #[tokio::test]
    async fn coach_without_bookings_gets_an_empty_list_without_asking_for_display_names() {
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
            slot(1, COACH, 10),
            slot(2, OTHER_COACH, 10),
        ]));
        book(&bookings, STUDENT, 2, 0).await;
        let lookup = RecordingLookup::default();
        let list = ListCoachBookings::new(bookings, lookup.clone(), clock())
            .execute(COACH, ListCoachBookingsQuery::default())
            .await
            .unwrap();
        assert!(list.items.is_empty());
        assert!(list.students.is_empty());
        assert!(lookup.asked.lock().unwrap().is_empty());
    }

    /// `COACH` の枠を 11 月 1 日 23 時・2 日 0 時・8 日 23 時・9 日 0 時・20 日 10 時（日本時間）に置き、どれも予約する。
    /// 別のコーチの 3 日の枠への予約も 1 件ある。
    async fn bookings_around_next_week() -> InMemoryBookingRepository {
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
            slot(1, COACH, hour_of(1, 23)),
            slot(2, COACH, hour_of(2, 0)),
            slot(3, COACH, hour_of(8, 23)),
            slot(4, COACH, hour_of(9, 0)),
            slot(5, COACH, hour_of(20, 10)),
            slot(6, OTHER_COACH, hour_of(3, 10)),
        ]));
        for id in 1..=6 {
            book(&bookings, STUDENT, id, 0).await;
        }
        bookings
    }

    async fn slot_ids(query: ListCoachBookingsQuery) -> Result<Vec<u128>, ListBookingsError> {
        let list = ListCoachBookings::new(
            bookings_around_next_week().await,
            RecordingLookup::default(),
            clock(),
        )
        .execute(COACH, query)
        .await?;
        Ok(list
            .items
            .iter()
            .map(|(_, slot)| slot.id().as_u128())
            .collect())
    }

    #[tokio::test]
    async fn without_dates_lists_every_booking_as_before() {
        let list = ListCoachBookings::new(
            bookings_around_next_week().await,
            RecordingLookup::default(),
            clock(),
        )
        .execute(COACH, ListCoachBookingsQuery::default())
        .await
        .unwrap();
        assert_eq!(
            list.items
                .iter()
                .map(|(_, slot)| slot.id().as_u128())
                .collect::<Vec<_>>(),
            [1, 2, 3, 4, 5]
        );
        assert_eq!((list.from, list.to), (None, None), "期間で絞らない");
    }

    #[tokio::test]
    async fn dates_narrow_to_bookings_whose_slot_starts_within_them_in_japan_time() {
        // 1 日 23 時と 9 日 0 時（日本時間）の枠は入らない。別のコーチの枠も出ない
        assert_eq!(slot_ids(query_from_to(2, 8)).await.unwrap(), [2, 3]);
        let list = ListCoachBookings::new(
            bookings_around_next_week().await,
            RecordingLookup::default(),
            clock(),
        )
        .execute(COACH, query_from_to(2, 8))
        .await
        .unwrap();
        assert_eq!((list.from, list.to), (Some(date(2)), Some(date(8))));
    }

    fn query_from_to(from: u8, to: u8) -> ListCoachBookingsQuery {
        ListCoachBookingsQuery {
            from: Some(date(from)),
            to: Some(date(to)),
        }
    }

    #[tokio::test]
    async fn omitted_from_or_to_follow_the_same_rule_as_list_bookings() {
        // to だけなら from は今日（2 日）
        let to_only = ListCoachBookingsQuery {
            from: None,
            to: Some(date(9)),
        };
        assert_eq!(slot_ids(to_only).await.unwrap(), [2, 3, 4]);
        // from だけなら to は from から 7 日目（9 日から 15 日）
        let from_only = ListCoachBookingsQuery {
            from: Some(date(9)),
            to: None,
        };
        let list = ListCoachBookings::new(
            bookings_around_next_week().await,
            RecordingLookup::default(),
            clock(),
        )
        .execute(COACH, from_only)
        .await
        .unwrap();
        assert_eq!((list.from, list.to), (Some(date(9)), Some(date(15))));
        assert_eq!(
            list.items
                .iter()
                .map(|(_, slot)| slot.id().as_u128())
                .collect::<Vec<_>>(),
            [4]
        );
    }

    #[tokio::test]
    async fn rejects_dates_that_break_the_period_rule() {
        assert!(matches!(
            slot_ids(query_from_to(8, 2)).await,
            Err(ListBookingsError::Invalid(
                SchedulingError::BookingPeriodEndsBeforeItStarts
            ))
        ));
        assert!(matches!(
            slot_ids(ListCoachBookingsQuery {
                from: None,
                to: Some(date(1)),
            })
            .await,
            Err(ListBookingsError::Invalid(
                SchedulingError::BookingPeriodEndsBeforeItStarts
            ))
        ));
        let too_long = ListCoachBookingsQuery {
            from: Some(date(1)),
            to: Some(Date::from_calendar_date(2026, Month::December, 2).unwrap()),
        };
        assert!(matches!(
            slot_ids(too_long).await,
            Err(ListBookingsError::Invalid(
                SchedulingError::BookingPeriodTooLong { max_days: 31 }
            ))
        ));
    }
}
