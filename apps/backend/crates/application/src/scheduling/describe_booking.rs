use std::collections::BTreeSet;

use domain::scheduling::Booking;

use super::{BookingCoachDisplayNameLookup, BookingRepository, DescribedBooking};
use crate::Unavailable;

/// 生徒が作った・確定した・キャンセルした予約に、予約したスロットとコーチの表示名を添えるユースケース（student-app の
/// `createBooking`・`confirmBooking`・`cancelBooking` が、返す予約に終わりの時刻・スポーツ・コーチの表示名を載せるのに使う）。
///
/// スロットは予約の保存先から、コーチの表示名は profile に問い合わせて（[`BookingCoachDisplayNameLookup`]）読む。
/// 予約のスロットが保存されていなければ [`Unavailable`]。読むだけで何も変えない。
pub struct DescribeBooking<B, K> {
    bookings: B,
    coaches: K,
}

impl<B, K> DescribeBooking<B, K>
where
    B: BookingRepository,
    K: BookingCoachDisplayNameLookup,
{
    pub fn new(bookings: B, coaches: K) -> Self {
        Self { bookings, coaches }
    }

    pub async fn execute(&self, booking: Booking) -> Result<DescribedBooking, Unavailable> {
        let slot = self
            .bookings
            .find_slot(booking.slot_id())
            .await?
            .ok_or_else(|| Unavailable::new("予約のスロットが保存されていない"))?;
        let coach_display_name = self
            .coaches
            .find_coach_display_names(&BTreeSet::from([booking.coach_id()]))
            .await?
            .remove(&booking.coach_id());
        Ok(DescribedBooking {
            booking,
            slot,
            coach_display_name,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        time::{Duration, SystemTime},
    };

    use domain::{
        catalog::Sport,
        identity::AccountId,
        profile::DisplayName,
        scheduling::{Slot, SlotId, SlotSeats, SlotStatus},
    };

    use super::*;
    use crate::in_memory::{InMemoryBookingRepository, InMemorySlotRepository};

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);
    const START: SystemTime = SystemTime::UNIX_EPOCH;
    const HOUR: Duration = Duration::from_secs(3600);

    fn slot(id: u128, coach: AccountId) -> Slot {
        Slot::restore(
            SlotId::from_u128(id),
            coach,
            Sport::Soccer,
            START + 48 * HOUR,
            START + 49 * HOUR,
            SlotStatus::Open,
            SlotSeats::default(),
        )
        .unwrap()
    }

    /// `COACH` の表示名だけを知っている。
    struct Names;

    impl BookingCoachDisplayNameLookup for Names {
        async fn find_coach_display_names(
            &self,
            coach_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            Ok(coach_ids
                .iter()
                .filter(|id| **id == COACH)
                .map(|id| (*id, DisplayName::new("たなか").unwrap()))
                .collect())
        }
    }

    async fn book(bookings: &InMemoryBookingRepository, slot: u128) -> Booking {
        bookings
            .reserve(SlotId::from_u128(slot), STUDENT, |slot, existing| {
                Booking::reserve(bookings.next_id(), STUDENT, false, slot, existing, START)
            })
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn adds_the_slot_and_the_coach_display_name() {
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
            slot(1, COACH),
            slot(2, OTHER_COACH),
        ]));
        let booking = book(&bookings, 1).await;
        let other = book(&bookings, 2).await;
        let describe = DescribeBooking::new(bookings, Names);

        let described = describe.execute(booking.clone()).await.unwrap();
        assert_eq!(described.booking, booking);
        assert_eq!(described.slot.id(), SlotId::from_u128(1));
        assert_eq!(described.slot.ends_at(), START + 49 * HOUR);
        assert_eq!(described.slot.sport(), Sport::Soccer);
        assert_eq!(
            described.coach_display_name,
            Some(DisplayName::new("たなか").unwrap())
        );

        let described = describe.execute(other).await.unwrap();
        assert_eq!(
            described.coach_display_name, None,
            "表示名が引けないコーチは None"
        );
    }

    #[tokio::test]
    async fn booking_without_a_saved_slot_is_unavailable() {
        let bookings =
            InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([slot(1, COACH)]));
        let booking = book(&bookings, 1).await;
        let describe = DescribeBooking::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            Names,
        );
        assert!(describe.execute(booking).await.is_err());
    }
}
