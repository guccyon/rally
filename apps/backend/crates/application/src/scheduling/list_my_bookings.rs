use std::{cmp::Reverse, collections::BTreeSet};

use domain::identity::AccountId;

use super::{BookingRepository, CoachIntroductionLookup, MyBookingList};
use crate::Unavailable;

/// 本人の予約の一覧のユースケース（student-app の `listMyBookings`。生徒だけが呼ぶ）。
///
/// 生徒 `student_id` の予約を、キャンセル済みも含めて予約したスロットと一緒に読み、スロットの開始時刻の新しい順に
/// 並べる（同じ開始時刻は予約の作成の新しい順、それも同じなら予約 ID の順）。一覧に出るコーチの紹介は profile に
/// 問い合わせて（[`CoachIntroductionLookup`]）添える。読むだけで何も変えない。
pub struct ListMyBookings<B, I> {
    bookings: B,
    coaches: I,
}

impl<B, I> ListMyBookings<B, I>
where
    B: BookingRepository,
    I: CoachIntroductionLookup,
{
    pub fn new(bookings: B, coaches: I) -> Self {
        Self { bookings, coaches }
    }

    pub async fn execute(&self, student_id: AccountId) -> Result<MyBookingList, Unavailable> {
        let mut items = self.bookings.bookings_of_student(student_id).await?;
        items.sort_by_key(|(booking, slot)| {
            (
                Reverse(slot.starts_at()),
                Reverse(booking.created_at()),
                booking.id(),
            )
        });
        let coach_ids: BTreeSet<AccountId> =
            items.iter().map(|(_, slot)| slot.coach_id()).collect();
        let coaches = if coach_ids.is_empty() {
            Default::default()
        } else {
            self.coaches.find_coach_introductions(&coach_ids).await?
        };
        Ok(MyBookingList { items, coaches })
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
        profile::{CoachBio, DisplayName},
        scheduling::{Booking, BookingStatus, Slot, SlotId, SlotStatus},
    };

    use super::*;
    use crate::{
        in_memory::{InMemoryBookingRepository, InMemorySlotRepository},
        scheduling::CoachIntroduction,
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

    fn introduction() -> CoachIntroduction {
        CoachIntroduction {
            display_name: DisplayName::new("たなか").unwrap(),
            bio: CoachBio::default(),
        }
    }

    /// `COACH` の紹介だけを知っていて（`OTHER_COACH` は紹介が引けない）、問い合わせた ID を記録する。
    #[derive(Clone, Default)]
    struct RecordingLookup {
        asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
    }

    impl CoachIntroductionLookup for RecordingLookup {
        async fn find_coach_introductions(
            &self,
            coach_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, CoachIntroduction>, Unavailable> {
            self.asked.lock().unwrap().push(coach_ids.clone());
            Ok(coach_ids
                .iter()
                .filter(|id| **id == COACH)
                .map(|id| (*id, introduction()))
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

    fn ids(list: &MyBookingList) -> Vec<(u128, BookingStatus)> {
        list.items
            .iter()
            .map(|(booking, slot)| (slot.id().as_u128(), booking.status()))
            .collect()
    }

    #[tokio::test]
    async fn lists_own_bookings_including_cancelled_ones_newest_slot_first() {
        let slots = InMemorySlotRepository::with_slots([
            slot(1, COACH, 10),
            slot(2, OTHER_COACH, 30),
            slot(3, COACH, 20),
            slot(4, COACH, 40),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        book(&bookings, STUDENT, 1, 0).await;
        let cancelled = book(&bookings, STUDENT, 3, 1).await;
        bookings
            .update(cancelled.id(), |booking, slot| {
                booking.cancel(STUDENT, slot, at(2)).map(|_| ())
            })
            .await
            .unwrap();
        // キャンセルした枠を取り直した予約は、キャンセル済みより前に出る
        book(&bookings, STUDENT, 3, 3).await;
        book(&bookings, STUDENT, 2, 4).await;
        // 別の生徒の予約は出ない
        book(&bookings, OTHER_STUDENT, 4, 5).await;

        let lookup = RecordingLookup::default();
        let list = ListMyBookings::new(bookings, lookup.clone())
            .execute(STUDENT)
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
        assert!(list.items.iter().all(
            |(booking, slot)| booking.student_id() == STUDENT && booking.slot_id() == slot.id()
        ));
        assert_eq!(
            list.coaches,
            BTreeMap::from([(COACH, introduction())]),
            "紹介が引けないコーチ（OTHER_COACH）は含まない"
        );
        assert_eq!(
            *lookup.asked.lock().unwrap(),
            [BTreeSet::from([COACH, OTHER_COACH])],
            "一覧のコーチを重ねずに 1 回で問い合わせる"
        );
    }

    #[tokio::test]
    async fn student_without_bookings_gets_an_empty_list_without_asking_for_introductions() {
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([slot(
            1, COACH, 10,
        )]));
        book(&bookings, OTHER_STUDENT, 1, 0).await;
        let lookup = RecordingLookup::default();
        let list = ListMyBookings::new(bookings, lookup.clone())
            .execute(STUDENT)
            .await
            .unwrap();
        assert!(list.items.is_empty());
        assert!(list.coaches.is_empty());
        assert!(lookup.asked.lock().unwrap().is_empty());
    }
}
