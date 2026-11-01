use std::{
    collections::{BTreeMap, BTreeSet},
    time::SystemTime,
};

use domain::{catalog::LessonMenuId, scheduling::RecentLessonMenuBookings};

use super::BookingRepository;
use crate::{Unavailable, catalog::LessonMenuRecentBookingCountLookup};

/// catalog の [`LessonMenuRecentBookingCountLookup`] の scheduling による実装。予約の保存先から期間の始まり以後に作られた
/// 予約を読み、数えるかの判定と数え方は domain の [`RecentLessonMenuBookings`] に任せる。何も変えない。
#[derive(Debug, Clone)]
pub struct SchedulingLessonMenuRecentBookingCountLookup<B> {
    bookings: B,
}

impl<B> SchedulingLessonMenuRecentBookingCountLookup<B> {
    pub fn new(bookings: B) -> Self {
        Self { bookings }
    }
}

impl<B> LessonMenuRecentBookingCountLookup for SchedulingLessonMenuRecentBookingCountLookup<B>
where
    B: BookingRepository,
{
    async fn count_recent_bookings(
        &self,
        lesson_menu_ids: &BTreeSet<LessonMenuId>,
        now: SystemTime,
    ) -> Result<BTreeMap<LessonMenuId, u32>, Unavailable> {
        if lesson_menu_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let bookings = self
            .bookings
            .bookings_of_lesson_menus_created_since(
                lesson_menu_ids,
                RecentLessonMenuBookings::since(now),
            )
            .await?;
        Ok(RecentLessonMenuBookings::tally(&bookings, now)
            .into_iter()
            .filter(|(id, _)| lesson_menu_ids.contains(id))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use domain::{
        catalog::{
            LessonDuration, LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription,
            LessonMenuName, Sport,
        },
        identity::AccountId,
        lesson::LessonType,
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

    fn menu(id: u128) -> LessonMenu {
        LessonMenu::restore(
            LessonMenuId::from_u128(id),
            COACH,
            LessonMenuContent {
                sport: Sport::Tennis,
                format: LessonFormat::new(
                    LessonType::Live,
                    Some(LessonDuration::from_minutes(60).unwrap()),
                )
                .unwrap(),
                name: LessonMenuName::new(format!("メニュー{id}")).unwrap(),
                description: LessonMenuDescription::default(),
            },
            true,
            SystemTime::UNIX_EPOCH,
        )
    }

    /// 同期レッスン 60 分のメニュー `menu_id` を結びつけた枠 `slot_id`（開始は照会した時刻の翌日）。
    fn slot(slot_id: u128, menu_id: Option<u128>) -> Slot {
        let slot = Slot::restore(
            SlotId::from_u128(slot_id),
            COACH,
            Sport::Tennis,
            now() + DAY,
            now() + DAY + Duration::from_secs(3600),
            SlotStatus::Open,
            SlotSeats::default(),
        )
        .unwrap();
        match menu_id {
            Some(menu_id) => slot.with_lesson_menu(&menu(menu_id)).unwrap(),
            None => slot,
        }
    }

    /// 生徒 `student` が枠 `slot_id` を `created_at` に予約する（枠のメニューを写す）。
    async fn book(
        bookings: &InMemoryBookingRepository,
        id: u128,
        student: u128,
        slot_id: u128,
        menu_id: Option<u128>,
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
                .map(|booking| booking.with_lesson_menu(slot, menu_id.map(menu).as_ref()))
            })
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn counts_active_bookings_of_the_last_thirty_days_per_asked_menu() {
        let slots = InMemorySlotRepository::with_slots([
            slot(1, Some(5)),
            slot(2, Some(5)),
            slot(3, Some(5)),
            slot(4, Some(5)),
            slot(5, Some(6)),
            slot(6, Some(9)),
            slot(7, None),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        book(&bookings, 1, 101, 1, Some(5), now() - 30 * DAY).await;
        book(&bookings, 2, 102, 2, Some(5), now()).await;
        // 期間の外とキャンセル済みは数えない
        book(&bookings, 3, 103, 3, Some(5), now() - 31 * DAY).await;
        let cancelled = book(&bookings, 4, 104, 4, Some(5), now() - DAY).await;
        bookings
            .update(cancelled.id(), |booking, slot| {
                booking
                    .cancel(AccountId::from_u128(104), slot, now())
                    .map(|_| ())
            })
            .await
            .unwrap();
        book(&bookings, 5, 105, 5, Some(6), now() - DAY).await;
        // 聞いていないメニューとメニューのない予約は結果に入らない
        book(&bookings, 6, 106, 6, Some(9), now() - DAY).await;
        book(&bookings, 7, 107, 7, None, now() - DAY).await;

        let lookup = SchedulingLessonMenuRecentBookingCountLookup::new(bookings);
        let ids = [5, 6, 8].map(LessonMenuId::from_u128).into();
        assert_eq!(
            lookup.count_recent_bookings(&ids, now()).await.unwrap(),
            BTreeMap::from([
                (LessonMenuId::from_u128(5), 2),
                (LessonMenuId::from_u128(6), 1)
            ]),
            "予約のないメニュー（8）は含めない"
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
