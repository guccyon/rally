use domain::scheduling::{Booking, BookingId, Slot, SlotId};

use super::BookingRepository;
use crate::{Unavailable, live_lesson::BookingLookup};

/// live-lesson の [`BookingLookup`] の scheduling による実装。予約の保存先から読むだけで、何も変えない。
#[derive(Debug, Clone)]
pub struct SchedulingBookingLookup<B> {
    bookings: B,
}

impl<B> SchedulingBookingLookup<B> {
    pub fn new(bookings: B) -> Self {
        Self { bookings }
    }
}

impl<B> BookingLookup for SchedulingBookingLookup<B>
where
    B: BookingRepository,
{
    async fn find_booking(&self, booking_id: BookingId) -> Result<Option<Booking>, Unavailable> {
        self.bookings.find(booking_id).await
    }

    async fn find_slot_bookings(&self, booking: &Booking) -> Result<Vec<Booking>, Unavailable> {
        // 枠のコーチの予約から同じ枠のものを選ぶ（予約の保存先に枠ごとの読み取りを足さない）
        let slot_id = booking.slot_id();
        Ok(self
            .bookings
            .bookings_of_coach(booking.coach_id())
            .await?
            .into_iter()
            .map(|(booking, _)| booking)
            .filter(|booking| booking.slot_id() == slot_id)
            .collect())
    }

    async fn find_slot(&self, slot_id: SlotId) -> Result<Option<Slot>, Unavailable> {
        self.bookings.find_slot(slot_id).await
    }
}
