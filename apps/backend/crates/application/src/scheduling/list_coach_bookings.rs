use std::{cmp::Reverse, collections::BTreeSet};

use domain::identity::AccountId;

use super::{BookingRepository, CoachBookingList, StudentDisplayNameLookup};
use crate::Unavailable;

/// コーチの自分の予約の一覧のユースケース（admin-web の `listMyCoachBookings`。コーチだけが呼ぶ）。
///
/// コーチ `coach_id` のスロットへの予約を、キャンセル済みも含めて予約されたスロットと一緒に読み、スロットの開始時刻の
/// 古い順に並べる（同じ開始時刻は予約の作成の新しい順、それも同じなら予約 ID の順）。一覧に出る生徒の表示名は identity に
/// 問い合わせて（[`StudentDisplayNameLookup`]）添える。読むだけで何も変えない。
pub struct ListCoachBookings<B, D> {
    bookings: B,
    students: D,
}

impl<B, D> ListCoachBookings<B, D>
where
    B: BookingRepository,
    D: StudentDisplayNameLookup,
{
    pub fn new(bookings: B, students: D) -> Self {
        Self { bookings, students }
    }

    pub async fn execute(&self, coach_id: AccountId) -> Result<CoachBookingList, Unavailable> {
        let mut items = self.bookings.bookings_of_coach(coach_id).await?;
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
        Ok(CoachBookingList { items, students })
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
        identity::{DisplayName, Sport},
        scheduling::{Booking, BookingStatus, Slot, SlotId, SlotStatus},
    };

    use super::*;
    use crate::{
        in_memory::{InMemoryBookingRepository, InMemorySlotRepository},
        scheduling::BookingRepository,
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(hour * 3600)
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
        let list = ListCoachBookings::new(bookings, lookup.clone())
            .execute(COACH)
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
        let list = ListCoachBookings::new(bookings, lookup.clone())
            .execute(COACH)
            .await
            .unwrap();
        assert!(list.items.is_empty());
        assert!(list.students.is_empty());
        assert!(lookup.asked.lock().unwrap().is_empty());
    }
}
