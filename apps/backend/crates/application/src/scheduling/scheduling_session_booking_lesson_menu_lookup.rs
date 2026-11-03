use std::collections::{BTreeMap, BTreeSet};

use domain::{catalog::LessonMenuId, scheduling::BookingId};

use super::BookingRepository;
use crate::{Unavailable, live_lesson::SessionBookingLessonMenuLookup};

/// live-lesson の [`SessionBookingLessonMenuLookup`] の scheduling による実装。予約の保存先から予約を読み、予約した時点の
/// メニューの写し（[`domain::scheduling::Booking::lesson_menu`]）の ID を返す。何も変えない。
#[derive(Debug, Clone)]
pub struct SchedulingSessionBookingLessonMenuLookup<B> {
    bookings: B,
}

impl<B> SchedulingSessionBookingLessonMenuLookup<B> {
    pub fn new(bookings: B) -> Self {
        Self { bookings }
    }
}

impl<B> SessionBookingLessonMenuLookup for SchedulingSessionBookingLessonMenuLookup<B>
where
    B: BookingRepository,
{
    async fn find_booking_lesson_menus(
        &self,
        booking_ids: &BTreeSet<BookingId>,
    ) -> Result<BTreeMap<BookingId, LessonMenuId>, Unavailable> {
        if booking_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        Ok(self
            .bookings
            .find_many(booking_ids)
            .await?
            .into_iter()
            .filter_map(|booking| booking.lesson_menu().map(|menu| (booking.id(), menu.id())))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::{
            LessonDuration, LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription,
            LessonMenuName, Sport,
        },
        identity::AccountId,
        lesson::LessonType,
        scheduling::{Booking, Slot, SlotId, SlotSeats, SlotStatus},
    };

    use super::*;
    use crate::in_memory::{InMemoryBookingRepository, InMemorySlotRepository};

    const COACH: AccountId = AccountId::from_u128(7);
    const MENU: LessonMenuId = LessonMenuId::from_u128(50);

    fn menu() -> LessonMenu {
        LessonMenu::restore(
            MENU,
            COACH,
            LessonMenuContent {
                sport: Sport::Tennis,
                format: LessonFormat::new(
                    LessonType::Live,
                    Some(LessonDuration::from_minutes(60).unwrap()),
                )
                .unwrap(),
                name: LessonMenuName::new("フォーム").unwrap(),
                description: LessonMenuDescription::default(),
            },
            true,
            SystemTime::UNIX_EPOCH,
        )
    }

    /// 枠 `id`（`with_menu` ならメニュー `MENU` を結びつける）を生徒 `id` が予約した予約 `id`（枠のメニューを写す）。
    async fn book(bookings: &InMemoryBookingRepository, id: u128, with_menu: bool) {
        let student = AccountId::from_u128(100 + id);
        bookings
            .reserve(SlotId::from_u128(id), student, |slot, existing| {
                Booking::reserve(
                    BookingId::from_u128(id),
                    student,
                    false,
                    slot,
                    existing,
                    SystemTime::UNIX_EPOCH,
                )
                .map(|booking| booking.with_lesson_menu(slot, with_menu.then(menu).as_ref()))
            })
            .await
            .unwrap();
    }

    fn slot(id: u128, with_menu: bool) -> Slot {
        let starts_at = SystemTime::UNIX_EPOCH + Duration::from_secs(86_400);
        let slot = Slot::restore(
            SlotId::from_u128(id),
            COACH,
            Sport::Tennis,
            starts_at,
            starts_at + Duration::from_secs(3600),
            SlotStatus::Open,
            SlotSeats::default(),
        )
        .unwrap();
        if with_menu {
            slot.with_lesson_menu(&menu()).unwrap()
        } else {
            slot
        }
    }

    #[tokio::test]
    async fn returns_the_menus_of_the_asked_bookings_that_have_one() {
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
            slot(1, true),
            slot(2, false),
            slot(3, true),
        ]));
        book(&bookings, 1, true).await;
        book(&bookings, 2, false).await;
        book(&bookings, 3, true).await;
        let lookup = SchedulingSessionBookingLessonMenuLookup::new(bookings);
        assert_eq!(
            lookup
                .find_booking_lesson_menus(&BTreeSet::from([
                    BookingId::from_u128(1),
                    BookingId::from_u128(2),
                    BookingId::from_u128(99),
                ]))
                .await
                .unwrap(),
            BTreeMap::from([(BookingId::from_u128(1), MENU)]),
            "メニューのない予約・ない ID・問い合わせていない予約は含めない"
        );
        assert!(
            lookup
                .find_booking_lesson_menus(&BTreeSet::new())
                .await
                .unwrap()
                .is_empty()
        );
    }
}
