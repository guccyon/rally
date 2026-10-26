use domain::scheduling::{Booking, BookingId};

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
}
