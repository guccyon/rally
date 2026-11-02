use std::collections::BTreeSet;

use domain::{catalog::LessonMenuId, identity::AccountId, scheduling::SlotId};
use time::Date;

use super::{
    BookingRepository, CoachSlotList, LessonMenuLookup, ListBookingsError, SlotRepository,
    calendar_day,
};
use crate::clock::Clock;

/// コーチの自分の枠の一覧のユースケース（admin-web の `listMyCoachSlots`。コーチだけが呼ぶ）。
///
/// コーチ `coach_id` の枠を、状態を問わず（終わった枠も含めて）読み、開始時刻の古い順に並べる（同じ開始時刻は枠の ID の
/// 順）。条件の `from`・`to` を両方省けば期間で絞らない。どちらかがあれば、日付（日本時間）の `from` から `to` まで
/// （両端を含む）に開始する枠だけに絞る。省いた側の決め方と期間の規則は本部が見る予約の一覧（[`super::ListBookings`]）と
/// 同じ [`calendar_day::forward_period`]（`from` を省けば今日、`to` を省けば `from` から 7 日目。31 日まで）。
/// それぞれの枠に有効な予約（[`domain::scheduling::BookingStatus::is_active`]。未確定か確定）が入っているかを、
/// コーチの枠への予約（[`BookingRepository::bookings_of_coach`]）から添える。枠に結びついたレッスンメニュー（名前と時間）は
/// catalog に問い合わせて（[`LessonMenuLookup`]）、公開中かを問わずに添える。読むだけで何も変えない。
pub struct ListCoachSlots<S, M, B, C> {
    slots: S,
    lesson_menus: M,
    bookings: B,
    clock: C,
}

/// コーチの自分の枠の一覧の条件。どれも省ける（両方省けば期間で絞らない）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ListCoachSlotsQuery {
    /// 期間の始まりの日付（日本時間。含む）
    pub from: Option<Date>,
    /// 期間の終わりの日付（日本時間。含む）
    pub to: Option<Date>,
}

impl<S, M, B, C> ListCoachSlots<S, M, B, C>
where
    S: SlotRepository,
    M: LessonMenuLookup,
    B: BookingRepository,
    C: Clock,
{
    pub fn new(slots: S, lesson_menus: M, bookings: B, clock: C) -> Self {
        Self {
            slots,
            lesson_menus,
            bookings,
            clock,
        }
    }

    pub async fn execute(
        &self,
        coach_id: AccountId,
        query: ListCoachSlotsQuery,
    ) -> Result<CoachSlotList, ListBookingsError> {
        let period = match (query.from, query.to) {
            (None, None) => None,
            (from, to) => Some(calendar_day::forward_period(from, to, self.clock.now())?.2),
        };
        let mut slots = self.slots.slots_of_coach(coach_id).await?;
        if let Some(period) = period {
            slots.retain(|slot| period.includes(slot.starts_at()));
        }
        slots.sort_by_key(|slot| (slot.starts_at(), slot.id()));
        let booked: BTreeSet<SlotId> = self
            .bookings
            .bookings_of_coach(coach_id)
            .await?
            .into_iter()
            .filter(|(booking, _)| booking.status().is_active())
            .map(|(booking, _)| booking.slot_id())
            .collect();
        let lesson_menu_ids: BTreeSet<LessonMenuId> = slots
            .iter()
            .filter_map(|slot| slot.lesson_menu_id())
            .collect();
        let lesson_menus = if lesson_menu_ids.is_empty() {
            Default::default()
        } else {
            self.lesson_menus
                .find_lesson_menus(&lesson_menu_ids)
                .await?
        };
        Ok(CoachSlotList {
            items: slots
                .into_iter()
                .map(|slot| {
                    let has_active_booking = booked.contains(&slot.id());
                    (slot, has_active_booking)
                })
                .collect(),
            lesson_menus,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        time::{Duration, SystemTime},
    };

    use domain::{
        catalog::Sport,
        scheduling::{Booking, Slot, SlotStatus},
    };

    use super::*;
    use crate::{
        catalog::CatalogLessonMenuLookup,
        in_memory::{
            FixedClock, InMemoryBookingRepository, InMemoryLessonMenuRepository,
            InMemorySlotRepository,
        },
    };

    fn no_menus() -> CatalogLessonMenuLookup<InMemoryLessonMenuRepository> {
        CatalogLessonMenuLookup::new(InMemoryLessonMenuRepository::default())
    }

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(hour * 3600)
    }

    fn slot(id: u128, coach: AccountId, hour: u64, status: SlotStatus) -> Slot {
        Slot::restore(
            SlotId::from_u128(id),
            coach,
            Sport::Tennis,
            at(hour),
            at(hour + 1),
            status,
            domain::scheduling::SlotSeats::default(),
        )
        .unwrap()
    }

    async fn book(bookings: &InMemoryBookingRepository, student: AccountId, slot: u128) -> Booking {
        bookings
            .reserve(SlotId::from_u128(slot), student, |slot, existing| {
                Booking::reserve(bookings.next_id(), student, false, slot, existing, at(0))
            })
            .await
            .unwrap()
    }

    fn ids(list: &CoachSlotList) -> Vec<(u128, bool)> {
        list.items
            .iter()
            .map(|(slot, has_active_booking)| (slot.id().as_u128(), *has_active_booking))
            .collect()
    }

    #[tokio::test]
    async fn lists_own_slots_of_any_status_oldest_first_with_whether_an_active_booking_is_in() {
        let slots = InMemorySlotRepository::with_slots([
            slot(1, COACH, 40, SlotStatus::Open),
            // 終わった枠（開始時刻が過去）も、閉じた枠も含む
            slot(2, COACH, 10, SlotStatus::Closed),
            slot(3, COACH, 30, SlotStatus::Open),
            slot(4, COACH, 20, SlotStatus::Open),
            // 同じ開始時刻は枠の ID の順
            slot(6, COACH, 50, SlotStatus::Open),
            slot(5, COACH, 50, SlotStatus::Open),
            slot(9, OTHER_COACH, 15, SlotStatus::Open),
        ]);
        let bookings = InMemoryBookingRepository::new(slots.clone());
        // 確定した予約
        let confirmed = book(&bookings, STUDENT, 1).await;
        bookings
            .update(confirmed.id(), |booking, slot| {
                booking.confirm(STUDENT, slot, at(1))
            })
            .await
            .unwrap();
        // 未確定の予約
        book(&bookings, STUDENT, 4).await;
        // キャンセル済みの予約だけの枠は、予約が入っていない
        let cancelled = book(&bookings, OTHER_STUDENT, 3).await;
        bookings
            .update(cancelled.id(), |booking, slot| {
                booking.cancel(OTHER_STUDENT, slot, at(2)).map(|_| ())
            })
            .await
            .unwrap();
        // 別のコーチの枠は出ない
        book(&bookings, STUDENT, 9).await;

        let list = ListCoachSlots::new(slots, no_menus(), bookings, FixedClock(at(0)))
            .execute(COACH, ListCoachSlotsQuery::default())
            .await
            .unwrap();
        assert_eq!(
            ids(&list),
            [
                (2, false),
                (4, true),
                (3, false),
                (1, true),
                (5, false),
                (6, false)
            ]
        );
        assert!(list.items.iter().all(|(slot, _)| slot.coach_id() == COACH));
        assert_eq!(list.items[3].0.status(), SlotStatus::Booked);
        assert!(list.lesson_menus.is_empty());
    }

    #[tokio::test]
    async fn adds_the_lesson_menus_bound_to_the_slots_whether_published_or_not() {
        use domain::catalog::{
            LessonDuration, LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription,
            LessonMenuName,
        };

        let menu = |id: u128, is_published| {
            LessonMenu::restore(
                LessonMenuId::from_u128(id),
                COACH,
                LessonMenuContent {
                    sport: Sport::Tennis,
                    format: LessonFormat::Live(LessonDuration::from_minutes(60).unwrap()),
                    name: LessonMenuName::new("フォア改善").unwrap(),
                    description: LessonMenuDescription::default(),
                },
                is_published,
                at(0),
            )
        };
        let bound = |id, hour, menu: u128| {
            slot(id, COACH, hour, SlotStatus::Open)
                .with_restored_lesson_menu(Some(LessonMenuId::from_u128(menu)))
        };
        let slots = InMemorySlotRepository::with_slots([
            bound(1, 10, 50),
            bound(2, 11, 51),
            bound(3, 12, 52),
            slot(4, COACH, 13, SlotStatus::Open),
        ]);
        let bookings = InMemoryBookingRepository::new(slots.clone());
        let menus = CatalogLessonMenuLookup::new(InMemoryLessonMenuRepository::with_menus([
            menu(50, true),
            menu(51, false),
            menu(53, true),
        ]));
        let list = ListCoachSlots::new(slots, menus, bookings, FixedClock(at(0)))
            .execute(COACH, ListCoachSlotsQuery::default())
            .await
            .unwrap();
        assert_eq!(
            list.items
                .iter()
                .map(|(slot, _)| slot.lesson_menu_id().map(LessonMenuId::as_u128))
                .collect::<Vec<_>>(),
            [Some(50), Some(51), Some(52), None]
        );
        assert_eq!(
            list.lesson_menus,
            BTreeMap::from([
                (LessonMenuId::from_u128(50), menu(50, true)),
                (LessonMenuId::from_u128(51), menu(51, false)),
            ]),
            "非公開のメニューも含め、ない（52）・結びついていない（53）メニューは含めない"
        );
    }

    #[tokio::test]
    async fn coach_without_slots_gets_an_empty_list() {
        let slots =
            InMemorySlotRepository::with_slots([slot(1, OTHER_COACH, 10, SlotStatus::Open)]);
        let bookings = InMemoryBookingRepository::new(slots.clone());
        let list = ListCoachSlots::new(slots, no_menus(), bookings, FixedClock(at(0)))
            .execute(COACH, ListCoachSlotsQuery::default())
            .await
            .unwrap();
        assert!(list.items.is_empty());
    }

    /// 2026 年 11 月 `day` 日の日本時間 `hour` 時
    fn jst(day: u8, hour: u8) -> SystemTime {
        calendar_day::start(nov(day)) + Duration::from_secs(u64::from(hour) * 3600)
    }

    fn nov(day: u8) -> Date {
        Date::from_calendar_date(2026, time::Month::November, day).unwrap()
    }

    fn slot_at(id: u128, coach: AccountId, starts_at: SystemTime) -> Slot {
        Slot::restore(
            SlotId::from_u128(id),
            coach,
            Sport::Tennis,
            starts_at,
            starts_at + Duration::from_secs(3600),
            SlotStatus::Open,
            domain::scheduling::SlotSeats::default(),
        )
        .unwrap()
    }

    /// 11 月 1 日の 0 時の直前・0 時・3 日の終わりの直前・4 日の 0 時・10 日の昼に始まる枠（開始の古い順に 1〜5）と、
    /// 別のコーチの 2 日の枠
    fn slots_around_november() -> InMemorySlotRepository {
        InMemorySlotRepository::with_slots([
            slot_at(1, COACH, jst(1, 0) - Duration::from_secs(1)),
            slot_at(2, COACH, jst(1, 0)),
            slot_at(3, COACH, jst(4, 0) - Duration::from_secs(1)),
            slot_at(4, COACH, jst(4, 0)),
            slot_at(5, COACH, jst(10, 12)),
            slot_at(9, OTHER_COACH, jst(2, 0)),
        ])
    }

    async fn list_with(
        now: SystemTime,
        query: ListCoachSlotsQuery,
    ) -> Result<Vec<u128>, ListBookingsError> {
        let slots = slots_around_november();
        let bookings = InMemoryBookingRepository::new(slots.clone());
        let list = ListCoachSlots::new(slots, no_menus(), bookings, FixedClock(now))
            .execute(COACH, query)
            .await?;
        Ok(list
            .items
            .iter()
            .map(|(slot, _)| slot.id().as_u128())
            .collect())
    }

    #[tokio::test]
    async fn without_dates_lists_every_slot_as_before() {
        assert_eq!(
            list_with(jst(2, 9), ListCoachSlotsQuery::default())
                .await
                .unwrap(),
            [1, 2, 3, 4, 5]
        );
    }

    #[tokio::test]
    async fn narrows_to_slots_starting_from_from_to_to_in_japan_time() {
        let query = ListCoachSlotsQuery {
            from: Some(nov(1)),
            to: Some(nov(3)),
        };
        assert_eq!(
            list_with(jst(20, 9), query).await.unwrap(),
            [2, 3],
            "1 日の日本時間 0 時（含む）から 4 日の 0 時（含まない）まで"
        );
    }

    #[tokio::test]
    async fn omitted_dates_follow_the_same_rule_as_the_bookings_list() {
        // from だけなら from から 7 日目まで（1〜7 日）
        let from_only = ListCoachSlotsQuery {
            from: Some(nov(1)),
            to: None,
        };
        assert_eq!(list_with(jst(20, 9), from_only).await.unwrap(), [2, 3, 4]);
        // to だけなら今日（4 日）から
        let to_only = ListCoachSlotsQuery {
            from: None,
            to: Some(nov(10)),
        };
        assert_eq!(list_with(jst(4, 9), to_only).await.unwrap(), [4, 5]);
    }

    #[tokio::test]
    async fn rejects_periods_that_break_the_period_rules() {
        let reversed = ListCoachSlotsQuery {
            from: Some(nov(3)),
            to: Some(nov(2)),
        };
        assert!(matches!(
            list_with(jst(1, 9), reversed).await,
            Err(ListBookingsError::Invalid(
                domain::scheduling::SchedulingError::BookingPeriodEndsBeforeItStarts
            ))
        ));
        // to だけで今日より前
        let before_today = ListCoachSlotsQuery {
            from: None,
            to: Some(nov(1)),
        };
        assert!(matches!(
            list_with(jst(2, 9), before_today).await,
            Err(ListBookingsError::Invalid(
                domain::scheduling::SchedulingError::BookingPeriodEndsBeforeItStarts
            ))
        ));
        let too_long = ListCoachSlotsQuery {
            from: Some(nov(1)),
            to: Some(Date::from_calendar_date(2026, time::Month::December, 2).unwrap()),
        };
        assert!(matches!(
            list_with(jst(1, 9), too_long).await,
            Err(ListBookingsError::Invalid(
                domain::scheduling::SchedulingError::BookingPeriodTooLong { max_days: 31 }
            ))
        ));
    }
}
