use std::collections::BTreeSet;

use domain::{identity::AccountId, scheduling::CoachBookedStudents};

use super::BookingRepository;
use crate::{Unavailable, identity::StudentCoachBookingLookup};

/// identity の [`StudentCoachBookingLookup`] の scheduling による実装。予約の保存先からコーチの枠への予約を読むだけで、
/// 何も変えない。予約したことがあるかの数え方（予約の状態を問わない）は domain の [`CoachBookedStudents`]。
#[derive(Debug, Clone)]
pub struct SchedulingStudentCoachBookingLookup<B> {
    bookings: B,
}

impl<B> SchedulingStudentCoachBookingLookup<B> {
    pub fn new(bookings: B) -> Self {
        Self { bookings }
    }
}

impl<B> StudentCoachBookingLookup for SchedulingStudentCoachBookingLookup<B>
where
    B: BookingRepository,
{
    async fn find_students_who_booked_coach(
        &self,
        coach_id: AccountId,
    ) -> Result<BTreeSet<AccountId>, Unavailable> {
        let bookings = self.bookings.bookings_of_coach(coach_id).await?;
        Ok(CoachBookedStudents::collect(
            bookings.iter().map(|(booking, _)| booking),
            coach_id,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::Sport,
        scheduling::{Booking, Slot, SlotId, SlotSeats, SlotStatus},
    };

    use super::*;
    use crate::in_memory::{InMemoryBookingRepository, InMemorySlotRepository};

    const STUDENT: AccountId = AccountId::from_u128(1);
    const CANCELLED_STUDENT: AccountId = AccountId::from_u128(2);
    const OTHER_COACH_STUDENT: AccountId = AccountId::from_u128(3);
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
            SlotSeats::default(),
        )
        .unwrap()
    }

    async fn book(bookings: &InMemoryBookingRepository, student: AccountId, slot: u128) -> Booking {
        bookings
            .reserve(SlotId::from_u128(slot), student, |slot, existing| {
                Booking::reserve(bookings.next_id(), student, false, slot, existing, at(10))
            })
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn returns_students_who_booked_the_coach_including_cancelled_bookings() {
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
            slot(1, COACH, 100),
            slot(2, COACH, 200),
            slot(3, OTHER_COACH, 300),
        ]));
        book(&bookings, STUDENT, 1).await;
        let cancelled = book(&bookings, CANCELLED_STUDENT, 2).await;
        bookings
            .update(cancelled.id(), |booking, slot| {
                booking.cancel(CANCELLED_STUDENT, slot, at(20)).map(|_| ())
            })
            .await
            .unwrap();
        book(&bookings, OTHER_COACH_STUDENT, 3).await;

        let lookup = SchedulingStudentCoachBookingLookup::new(bookings);
        assert_eq!(
            lookup.find_students_who_booked_coach(COACH).await.unwrap(),
            [STUDENT, CANCELLED_STUDENT].into(),
            "キャンセルした予約も数え、ほかのコーチだけを予約した生徒は含めない"
        );
        assert!(
            lookup
                .find_students_who_booked_coach(AccountId::from_u128(9))
                .await
                .unwrap()
                .is_empty(),
            "予約のないコーチ・ない ID なら空"
        );
    }
}
