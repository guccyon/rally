use domain::{identity::Sport, scheduling::SlotSupply};

use super::{BookingRepository, SlotSupplyReport};
use crate::{Unavailable, clock::Clock};

/// スポーツごとの予約枠の需給のユースケース（admin-web の `getSlotSupply`。本部だけが呼ぶ）。
///
/// 現在時刻の [`SlotSupply::WINDOW`] 前（含む）から現在時刻（含まない）までに開始する枠と、それらへの予約を
/// 読み、スポーツ 6 種類すべてについて [`SlotSupply::tally`] で数える。数え方と判定は domain の [`SlotSupply`] だけ。
pub struct GetSlotSupply<B, C> {
    bookings: B,
    clock: C,
}

impl<B, C> GetSlotSupply<B, C>
where
    B: BookingRepository,
    C: Clock,
{
    pub fn new(bookings: B, clock: C) -> Self {
        Self { bookings, clock }
    }

    pub async fn execute(&self) -> Result<SlotSupplyReport, Unavailable> {
        let window_end = self.clock.now();
        let window_start = window_end - SlotSupply::WINDOW;
        let (slots, bookings) = self
            .bookings
            .slots_starting_between(window_start, window_end)
            .await?;
        Ok(SlotSupplyReport {
            window_start,
            window_end,
            sports: Sport::ALL
                .into_iter()
                .map(|sport| SlotSupply::tally(sport, &slots, &bookings))
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        identity::AccountId,
        scheduling::{Booking, Slot, SlotId, SlotStatus, SlotSupplyStatus},
    };

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryBookingRepository, InMemorySlotRepository};

    const DAY: u64 = 24 * 3600;
    /// 現在時刻（100 日目の 0 時）
    const NOW: Duration = Duration::from_secs(100 * DAY);

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn slot(id: u128, sport: Sport, starts_at: SystemTime) -> Slot {
        Slot::restore(
            SlotId::from_u128(id),
            AccountId::from_u128(1000 + id),
            sport,
            starts_at,
            starts_at + Duration::from_secs(3600),
            SlotStatus::Open,
            domain::scheduling::SlotSeats::default(),
        )
        .unwrap()
    }

    async fn book(bookings: &InMemoryBookingRepository, student: u128, slot: u128) {
        let student = AccountId::from_u128(student);
        bookings
            .reserve(SlotId::from_u128(slot), student, |slot, existing| {
                Booking::reserve(bookings.next_id(), student, false, slot, existing, at(0))
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn counts_slots_starting_in_the_last_thirty_days_for_every_sport() {
        let now = SystemTime::UNIX_EPOCH + NOW;
        let window_start = now - Duration::from_secs(30 * DAY);
        let slots = InMemorySlotRepository::with_slots([
            // 期間の始まりちょうど（含む）と終わりの直前
            slot(1, Sport::Tennis, window_start),
            slot(2, Sport::Tennis, now - Duration::from_secs(1)),
            // 期間の外（始まりの直前と、現在時刻ちょうど）
            slot(3, Sport::Tennis, window_start - Duration::from_secs(1)),
            slot(4, Sport::Tennis, now),
            slot(5, Sport::Soccer, now - Duration::from_secs(DAY)),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        book(&bookings, 7, 1).await;
        book(&bookings, 8, 2).await;
        // 期間の外の枠への予約は数えない
        book(&bookings, 9, 3).await;
        book(&bookings, 10, 4).await;

        let report = GetSlotSupply::new(bookings, FixedClock(now))
            .execute()
            .await
            .unwrap();
        assert_eq!(
            (report.window_start, report.window_end),
            (window_start, now)
        );
        assert_eq!(
            report.sports,
            [
                SlotSupply::new(Sport::Tennis, 2, 2),
                SlotSupply::new(Sport::Golf, 0, 0),
                SlotSupply::new(Sport::Baseball, 0, 0),
                SlotSupply::new(Sport::Soccer, 1, 0),
                SlotSupply::new(Sport::Basketball, 0, 0),
                SlotSupply::new(Sport::Swimming, 0, 0),
            ]
        );
        let statuses: Vec<_> = report.sports.iter().map(SlotSupply::status).collect();
        assert_eq!(
            statuses,
            [
                SlotSupplyStatus::Insufficient,
                SlotSupplyStatus::Sufficient,
                SlotSupplyStatus::Sufficient,
                SlotSupplyStatus::Sufficient,
                SlotSupplyStatus::Sufficient,
                SlotSupplyStatus::Sufficient,
            ]
        );
    }
}
