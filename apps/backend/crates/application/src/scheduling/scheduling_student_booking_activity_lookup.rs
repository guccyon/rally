use std::time::SystemTime;

use domain::{identity::AccountId, scheduling::BookingStatus};

use super::BookingRepository;
use crate::{
    Unavailable,
    identity::{StudentBookingActivity, StudentBookingActivityLookup},
};

/// identity の [`StudentBookingActivityLookup`] の scheduling による実装。予約の保存先から生徒の予約を読むだけで、
/// 何も変えない。
///
/// これからの予約は、有効な予約（未確定・確定）のうち予約したスロットの開始が `now` より後のもの（始まった予約と
/// キャンセル済みの予約は含めない）。キャンセルの件数は、キャンセル済みの予約の数（期間で絞らない）。
#[derive(Debug, Clone)]
pub struct SchedulingStudentBookingActivityLookup<B> {
    bookings: B,
}

impl<B> SchedulingStudentBookingActivityLookup<B> {
    pub fn new(bookings: B) -> Self {
        Self { bookings }
    }
}

impl<B> StudentBookingActivityLookup for SchedulingStudentBookingActivityLookup<B>
where
    B: BookingRepository,
{
    async fn find_booking_activity(
        &self,
        student_id: AccountId,
        now: SystemTime,
    ) -> Result<StudentBookingActivity, Unavailable> {
        let bookings = self.bookings.bookings_of_student(student_id).await?;
        let cancelled = bookings
            .iter()
            .filter(|(booking, _)| booking.status() == BookingStatus::Cancelled)
            .count();
        let mut upcoming: Vec<_> = bookings
            .into_iter()
            .map(|(booking, _)| booking)
            .filter(|booking| booking.status().is_active() && booking.starts_at() > now)
            .collect();
        upcoming.sort_by_key(|booking| (booking.starts_at(), booking.created_at(), booking.id()));
        Ok(StudentBookingActivity {
            upcoming,
            cancelled_count: u32::try_from(cancelled)
                .map_err(|_| Unavailable::new("キャンセルの件数が数えられる範囲を超えた"))?,
        })
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

    async fn cancel(bookings: &InMemoryBookingRepository, booking: &Booking, hour: u64) {
        bookings
            .update(booking.id(), |booking, slot| {
                booking.cancel(STUDENT, slot, at(hour)).map(|_| ())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn upcoming_active_bookings_in_start_order_and_cancelled_count() {
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
            slot(1, 10),
            slot(2, 200),
            slot(3, 100),
            slot(4, 300),
            slot(5, 20),
            slot(6, 500),
            slot(7, 50),
        ]));
        // 始まった予約（開始が今と同じ・今より前）は含めない
        book(&bookings, STUDENT, 1, 1).await;
        book(&bookings, STUDENT, 7, 2).await;
        let later = book(&bookings, STUDENT, 2, 3).await;
        let sooner = book(&bookings, STUDENT, 3, 4).await;
        // キャンセルした予約は、これからでも始まっていても数えるだけ
        let cancelled_upcoming = book(&bookings, STUDENT, 4, 5).await;
        cancel(&bookings, &cancelled_upcoming, 6).await;
        let cancelled_past = book(&bookings, STUDENT, 5, 7).await;
        cancel(&bookings, &cancelled_past, 8).await;
        // 別の生徒の予約は含めない
        book(&bookings, OTHER_STUDENT, 6, 9).await;

        let lookup = SchedulingStudentBookingActivityLookup::new(bookings);
        let activity = lookup.find_booking_activity(STUDENT, at(50)).await.unwrap();
        assert_eq!(
            activity
                .upcoming
                .iter()
                .map(Booking::id)
                .collect::<Vec<_>>(),
            [sooner.id(), later.id()],
            "開始の早い順"
        );
        assert_eq!(activity.cancelled_count, 2);
    }

    #[tokio::test]
    async fn student_without_bookings_is_empty() {
        let lookup = SchedulingStudentBookingActivityLookup::new(InMemoryBookingRepository::new(
            InMemorySlotRepository::default(),
        ));
        assert_eq!(
            lookup.find_booking_activity(STUDENT, at(0)).await.unwrap(),
            StudentBookingActivity::default()
        );
    }
}
