use std::{
    collections::{BTreeMap, BTreeSet},
    time::SystemTime,
};

use domain::{identity::AccountId, scheduling::RecentStudentBookings};

use super::BookingRepository;
use crate::{Unavailable, identity::StudentRecentBookingCountLookup};

/// identity の [`StudentRecentBookingCountLookup`] の scheduling による実装。予約の保存先から期間の始まり以後に作られた
/// 生徒の予約を読み、数えるかの判定と数え方は domain の [`RecentStudentBookings`] に任せる。何も変えない。
#[derive(Debug, Clone)]
pub struct SchedulingStudentRecentBookingCountLookup<B> {
    bookings: B,
}

impl<B> SchedulingStudentRecentBookingCountLookup<B> {
    pub fn new(bookings: B) -> Self {
        Self { bookings }
    }
}

impl<B> StudentRecentBookingCountLookup for SchedulingStudentRecentBookingCountLookup<B>
where
    B: BookingRepository,
{
    async fn count_recent_bookings(
        &self,
        student_ids: &BTreeSet<AccountId>,
        now: SystemTime,
    ) -> Result<BTreeMap<AccountId, u32>, Unavailable> {
        if student_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let bookings = self
            .bookings
            .bookings_of_students_created_since(student_ids, RecentStudentBookings::since(now))
            .await?;
        Ok(RecentStudentBookings::tally(&bookings, now)
            .into_iter()
            .filter(|(id, _)| student_ids.contains(id))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use domain::{
        catalog::Sport,
        scheduling::{Booking, BookingId, Slot, SlotId, SlotSeats, SlotStatus},
    };

    use super::*;
    use crate::in_memory::{InMemoryBookingRepository, InMemorySlotRepository};

    const DAY: Duration = Duration::from_secs(24 * 3600);
    const COACH: AccountId = AccountId::from_u128(7);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    /// メニューのない枠 `slot_id`（開始は照会した時刻の翌日）。
    fn slot(slot_id: u128) -> Slot {
        Slot::restore(
            SlotId::from_u128(slot_id),
            COACH,
            Sport::Tennis,
            now() + DAY,
            now() + DAY + Duration::from_secs(3600),
            SlotStatus::Open,
            SlotSeats::default(),
        )
        .unwrap()
    }

    /// 生徒 `student` が枠 `slot_id` を `created_at` に予約する。
    async fn book(
        bookings: &InMemoryBookingRepository,
        id: u128,
        student: u128,
        slot_id: u128,
        created_at: SystemTime,
    ) -> Booking {
        let student = AccountId::from_u128(student);
        bookings
            .reserve(SlotId::from_u128(slot_id), student, |slot, existing| {
                Booking::reserve(
                    BookingId::from_u128(id),
                    student,
                    false,
                    slot,
                    existing,
                    created_at,
                )
            })
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn counts_active_bookings_of_the_last_thirty_days_per_asked_student() {
        let bookings =
            InMemoryBookingRepository::new(InMemorySlotRepository::with_slots((1..=6).map(slot)));
        book(&bookings, 1, 101, 1, now() - 30 * DAY).await;
        book(&bookings, 2, 101, 2, now()).await;
        // 期間の外とキャンセル済みは数えない
        book(&bookings, 3, 101, 3, now() - 31 * DAY).await;
        let cancelled = book(&bookings, 4, 102, 4, now() - DAY).await;
        bookings
            .update(cancelled.id(), |booking, slot| {
                booking
                    .cancel(AccountId::from_u128(102), slot, now())
                    .map(|_| ())
            })
            .await
            .unwrap();
        book(&bookings, 5, 103, 5, now() - DAY).await;
        // 聞いていない生徒の予約は結果に入らない
        book(&bookings, 6, 109, 6, now() - DAY).await;

        let lookup = SchedulingStudentRecentBookingCountLookup::new(bookings);
        let ids = [101, 102, 103, 104].map(AccountId::from_u128).into();
        assert_eq!(
            lookup.count_recent_bookings(&ids, now()).await.unwrap(),
            BTreeMap::from([
                (AccountId::from_u128(101), 2),
                (AccountId::from_u128(103), 1)
            ]),
            "キャンセルした予約しかない生徒（102）と予約のない生徒（104）は含めない"
        );
        assert!(
            lookup
                .count_recent_bookings(&BTreeSet::new(), now())
                .await
                .unwrap()
                .is_empty()
        );
    }
}
