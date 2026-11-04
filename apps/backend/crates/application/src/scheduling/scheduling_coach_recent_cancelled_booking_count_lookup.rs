use domain::{identity::AccountId, scheduling::CoachRecentCancelledBookingCount};

use super::BookingRepository;
use crate::{Unavailable, clock::Clock, profile::CoachRecentCancelledBookingCountLookup};

/// profile の [`CoachRecentCancelledBookingCountLookup`] の scheduling による実装。予約の保存先からコーチのスロットへの予約を
/// 読み、照会した時刻（[`Clock`]）で domain の [`CoachRecentCancelledBookingCount::tally`] で数える。何も変えない。
#[derive(Debug, Clone)]
pub struct SchedulingCoachRecentCancelledBookingCountLookup<B, C> {
    bookings: B,
    clock: C,
}

impl<B, C> SchedulingCoachRecentCancelledBookingCountLookup<B, C> {
    pub fn new(bookings: B, clock: C) -> Self {
        Self { bookings, clock }
    }
}

impl<B, C> CoachRecentCancelledBookingCountLookup
    for SchedulingCoachRecentCancelledBookingCountLookup<B, C>
where
    B: BookingRepository,
    C: Clock,
{
    async fn find_recent_cancelled_booking_count(
        &self,
        coach_id: AccountId,
    ) -> Result<CoachRecentCancelledBookingCount, Unavailable> {
        let now = self.clock.now();
        let bookings = self.bookings.bookings_of_coach(coach_id).await?;
        Ok(CoachRecentCancelledBookingCount::tally(
            coach_id,
            bookings.iter().map(|(booking, _)| booking),
            now,
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
    use crate::in_memory::{FixedClock, InMemoryBookingRepository, InMemorySlotRepository};

    const DAY: Duration = Duration::from_secs(24 * 3600);
    const SECOND: Duration = Duration::from_secs(1);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);
    const NO_BOOKINGS_COACH: AccountId = AccountId::from_u128(9);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    fn slot(id: u128, coach_id: AccountId) -> Slot {
        let starts_at = now() + DAY * u32::try_from(id).unwrap();
        Slot::restore(
            SlotId::from_u128(id),
            coach_id,
            Sport::Tennis,
            starts_at,
            starts_at + Duration::from_secs(3600),
            SlotStatus::Open,
            SlotSeats::default(),
        )
        .unwrap()
    }

    async fn book(bookings: &InMemoryBookingRepository, slot: u128) -> Booking {
        bookings
            .reserve(SlotId::from_u128(slot), STUDENT, |slot, existing| {
                Booking::reserve(
                    bookings.next_id(),
                    STUDENT,
                    false,
                    slot,
                    existing,
                    now() - 40 * DAY,
                )
            })
            .await
            .unwrap()
    }

    async fn cancel(bookings: &InMemoryBookingRepository, booking: &Booking, at: SystemTime) {
        bookings
            .update(booking.id(), |booking, slot| {
                booking.cancel(STUDENT, slot, at).map(|_| ())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn counts_bookings_of_the_coach_cancelled_in_the_last_30_days() {
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
            slot(1, COACH),
            slot(2, COACH),
            slot(3, COACH),
            slot(4, COACH),
            slot(5, OTHER_COACH),
        ]));
        for (slot, cancelled_at) in [
            (1, Some(now())),
            (2, Some(now() - 30 * DAY)),
            (3, Some(now() - 30 * DAY - SECOND)),
            (4, None),
            (5, Some(now() - DAY)),
        ] {
            let booking = book(&bookings, slot).await;
            if let Some(at) = cancelled_at {
                cancel(&bookings, &booking, at).await;
            }
        }

        let count = SchedulingCoachRecentCancelledBookingCountLookup::new(
            bookings.clone(),
            FixedClock(now()),
        )
        .find_recent_cancelled_booking_count(COACH)
        .await
        .unwrap();
        assert_eq!(count.coach_id(), COACH);
        assert_eq!(
            count.cancelled_count(),
            2,
            "30 日前ちょうどは含み、それより前のキャンセル・有効な予約・他のコーチの予約は数えない"
        );

        // 期間は照会した時刻から数える
        let later = SchedulingCoachRecentCancelledBookingCountLookup::new(
            bookings.clone(),
            FixedClock(now() + SECOND),
        )
        .find_recent_cancelled_booking_count(COACH)
        .await
        .unwrap();
        assert_eq!(later.cancelled_count(), 1);

        let none =
            SchedulingCoachRecentCancelledBookingCountLookup::new(bookings, FixedClock(now()))
                .find_recent_cancelled_booking_count(NO_BOOKINGS_COACH)
                .await
                .unwrap();
        assert_eq!(none.cancelled_count(), 0);
    }
}
