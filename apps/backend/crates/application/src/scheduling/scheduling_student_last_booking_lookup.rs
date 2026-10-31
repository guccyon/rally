use std::{
    collections::{BTreeMap, BTreeSet},
    time::SystemTime,
};

use domain::identity::AccountId;

use super::BookingRepository;
use crate::{Unavailable, identity::StudentLastBookingLookup};

/// identity の [`StudentLastBookingLookup`] の scheduling による実装。予約の保存先から読むだけで、何も変えない。
///
/// 最後に予約した時刻は予約を作った時刻の最も新しいもので、予約の状態（確定待ち・確定済み・キャンセル済み）は問わない
/// （本部が見たいのは生徒が最近予約しているかで、あとでキャンセルしても予約した事実は残る）。
#[derive(Debug, Clone)]
pub struct SchedulingStudentLastBookingLookup<B> {
    bookings: B,
}

impl<B> SchedulingStudentLastBookingLookup<B> {
    pub fn new(bookings: B) -> Self {
        Self { bookings }
    }
}

impl<B> StudentLastBookingLookup for SchedulingStudentLastBookingLookup<B>
where
    B: BookingRepository,
{
    async fn find_last_booked_at(
        &self,
        student_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, SystemTime>, Unavailable> {
        self.bookings.last_booked_at_of_students(student_ids).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use domain::{
        catalog::Sport,
        scheduling::{Booking, Slot, SlotId, SlotSeats, SlotStatus},
    };

    use super::*;
    use crate::in_memory::{InMemoryBookingRepository, InMemorySlotRepository};

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const NO_BOOKING_STUDENT: AccountId = AccountId::from_u128(3);
    const COACH: AccountId = AccountId::from_u128(7);

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(hour * 3600)
    }

    fn slot(id: u128, hour: u64) -> Slot {
        Slot::restore(
            SlotId::from_u128(id),
            COACH,
            Sport::Tennis,
            at(hour),
            at(hour + 1),
            SlotStatus::Open,
            SlotSeats::default(),
        )
        .unwrap()
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

    #[tokio::test]
    async fn returns_latest_booking_time_per_student_including_cancelled_ones() {
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
            slot(1, 100),
            slot(2, 200),
            slot(3, 300),
        ]));
        book(&bookings, STUDENT, 1, 10).await;
        // 後で作った予約は、開始が早い枠でも新しい
        let latest = book(&bookings, STUDENT, 3, 30).await;
        book(&bookings, OTHER_STUDENT, 2, 20).await;
        bookings
            .update(latest.id(), |booking, slot| {
                booking.cancel(STUDENT, slot, at(40)).map(|_| ())
            })
            .await
            .unwrap();

        let lookup = SchedulingStudentLastBookingLookup::new(bookings);
        let ids = [STUDENT, NO_BOOKING_STUDENT].into();
        assert_eq!(
            lookup.find_last_booked_at(&ids).await.unwrap(),
            BTreeMap::from([(STUDENT, at(30))]),
            "問い合わせていない生徒と、予約のない生徒は含めない。キャンセルした予約も数える"
        );
    }
}
