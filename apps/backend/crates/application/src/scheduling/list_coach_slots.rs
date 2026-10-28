use std::collections::BTreeSet;

use domain::{identity::AccountId, scheduling::SlotId};

use super::{BookingRepository, CoachSlotList, SlotRepository};
use crate::Unavailable;

/// コーチの自分の枠の一覧のユースケース（admin-web の `listMyCoachSlots`。コーチだけが呼ぶ）。
///
/// コーチ `coach_id` の枠を、状態を問わず（終わった枠も含めて）読み、開始時刻の古い順に並べる（同じ開始時刻は枠の ID の
/// 順）。それぞれの枠に有効な予約（[`domain::scheduling::BookingStatus::is_active`]。未確定か確定）が入っているかを、
/// コーチの枠への予約（[`BookingRepository::bookings_of_coach`]）から添える。読むだけで何も変えない。
pub struct ListCoachSlots<S, B> {
    slots: S,
    bookings: B,
}

impl<S, B> ListCoachSlots<S, B>
where
    S: SlotRepository,
    B: BookingRepository,
{
    pub fn new(slots: S, bookings: B) -> Self {
        Self { slots, bookings }
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
        Ok(CoachSlotList {
            items: slots
                .into_iter()
                .map(|slot| {
                    let has_active_booking = booked.contains(&slot.id());
                    (slot, has_active_booking)
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        identity::Sport,
        scheduling::{Booking, Slot, SlotStatus},
    };

    use super::*;
    use crate::in_memory::{InMemoryBookingRepository, InMemorySlotRepository};

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

        let list = ListCoachSlots::new(slots, bookings)
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
    }

    #[tokio::test]
    async fn coach_without_slots_gets_an_empty_list() {
        let slots =
            InMemorySlotRepository::with_slots([slot(1, OTHER_COACH, 10, SlotStatus::Open)]);
        let bookings = InMemoryBookingRepository::new(slots.clone());
        let list = ListCoachSlots::new(slots, bookings)
            .execute(COACH)
            .await
            .unwrap();
        assert!(list.items.is_empty());
    }
}
