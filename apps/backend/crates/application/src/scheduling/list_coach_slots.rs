use std::collections::BTreeSet;

use domain::{catalog::LessonMenuId, identity::AccountId, scheduling::SlotId};

use super::{BookingRepository, CoachSlotList, LessonMenuLookup, SlotRepository};
use crate::Unavailable;

/// コーチの自分の枠の一覧のユースケース（admin-web の `listMyCoachSlots`。コーチだけが呼ぶ）。
///
/// コーチ `coach_id` の枠を、状態を問わず（終わった枠も含めて）読み、開始時刻の古い順に並べる（同じ開始時刻は枠の ID の
/// 順）。それぞれの枠に有効な予約（[`domain::scheduling::BookingStatus::is_active`]。未確定か確定）が入っているかを、
/// コーチの枠への予約（[`BookingRepository::bookings_of_coach`]）から添える。枠に結びついたレッスンメニュー（名前と時間）は
/// catalog に問い合わせて（[`LessonMenuLookup`]）、公開中かを問わずに添える。読むだけで何も変えない。
pub struct ListCoachSlots<S, M, B> {
    slots: S,
    lesson_menus: M,
    bookings: B,
}

impl<S, M, B> ListCoachSlots<S, M, B>
where
    S: SlotRepository,
    M: LessonMenuLookup,
    B: BookingRepository,
{
    pub fn new(slots: S, lesson_menus: M, bookings: B) -> Self {
        Self {
            slots,
            lesson_menus,
            bookings,
        }
    }

    pub async fn execute(&self, coach_id: AccountId) -> Result<CoachSlotList, Unavailable> {
        let mut slots = self.slots.slots_of_coach(coach_id).await?;
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
            InMemoryBookingRepository, InMemoryLessonMenuRepository, InMemorySlotRepository,
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

        let list = ListCoachSlots::new(slots, no_menus(), bookings)
            .execute(COACH)
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
        let list = ListCoachSlots::new(slots, menus, bookings)
            .execute(COACH)
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
        let list = ListCoachSlots::new(slots, no_menus(), bookings)
            .execute(COACH)
            .await
            .unwrap();
        assert!(list.items.is_empty());
    }
}
