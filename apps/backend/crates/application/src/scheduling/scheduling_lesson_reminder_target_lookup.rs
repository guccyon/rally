use std::time::SystemTime;

use domain::scheduling::BookingStatus;

use super::BookingRepository;
use crate::{
    Unavailable,
    notification::{LessonReminderTarget, LessonReminderTargetLookup},
};

/// [`LessonReminderTargetLookup`] の実装（scheduling が notification に答える）。予約の保存先（[`BookingRepository`]）から、
/// 開始が期間に入る枠への予約を読み、確定しているものだけを返す。予約はどれもスロット（ビデオ通話の枠）への予約なので、
/// 同期レッスンの予約に限られる（非同期レッスンは動画の送付で、予約を持たない）。
pub struct SchedulingLessonReminderTargetLookup<B> {
    bookings: B,
}

impl<B> SchedulingLessonReminderTargetLookup<B> {
    pub fn new(bookings: B) -> Self {
        Self { bookings }
    }
}

impl<B> LessonReminderTargetLookup for SchedulingLessonReminderTargetLookup<B>
where
    B: BookingRepository,
{
    async fn confirmed_bookings_starting_between(
        &self,
        from: SystemTime,
        until: SystemTime,
    ) -> Result<Vec<LessonReminderTarget>, Unavailable> {
        let (_, bookings) = self.bookings.slots_starting_between(from, until).await?;
        Ok(bookings
            .into_iter()
            .filter(|booking| booking.status() == BookingStatus::Confirmed)
            .map(|booking| LessonReminderTarget {
                booking_id: booking.id(),
                student_id: booking.student_id(),
                coach_id: booking.coach_id(),
                starts_at: booking.starts_at(),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use domain::{
        catalog::Sport,
        identity::AccountId,
        scheduling::{Booking, Slot, SlotId, SlotSeats, SlotStatus},
    };

    use super::*;
    use crate::in_memory::{InMemoryBookingRepository, InMemorySlotRepository};

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(9);

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

    async fn book(bookings: &InMemoryBookingRepository, student: AccountId, slot: u128) -> Booking {
        bookings
            .reserve(SlotId::from_u128(slot), student, |slot, existing| {
                Booking::reserve(bookings.next_id(), student, false, slot, existing, at(0))
            })
            .await
            .unwrap()
    }

    async fn confirm(bookings: &InMemoryBookingRepository, booking: &Booking) -> Booking {
        let student = booking.student_id();
        bookings
            .update(booking.id(), |booking, slot| {
                booking.confirm(student, slot, at(0))
            })
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn only_confirmed_bookings_in_the_period_are_targets() {
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
            slot(1, 10),
            slot(2, 11),
            slot(3, 12),
            slot(4, 30),
        ]));
        let confirmed = book(&bookings, STUDENT, 1).await;
        let confirmed = confirm(&bookings, &confirmed).await;
        let cancelled = book(&bookings, OTHER_STUDENT, 2).await;
        let cancelled = confirm(&bookings, &cancelled).await;
        bookings
            .update(cancelled.id(), |booking, slot| {
                booking.cancel(OTHER_STUDENT, slot, at(1)).map(|_| ())
            })
            .await
            .unwrap();
        book(&bookings, STUDENT, 3).await;
        let outside = book(&bookings, OTHER_STUDENT, 4).await;
        confirm(&bookings, &outside).await;

        let targets = SchedulingLessonReminderTargetLookup::new(bookings)
            .confirmed_bookings_starting_between(at(10), at(24))
            .await
            .unwrap();

        assert_eq!(
            targets,
            [LessonReminderTarget {
                booking_id: confirmed.id(),
                student_id: STUDENT,
                coach_id: COACH,
                starts_at: at(10),
            }],
            "キャンセルした予約・確定待ち・期間の外は対象でない"
        );
    }
}
