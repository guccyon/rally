use std::collections::BTreeSet;

use domain::scheduling::BookingId;

use super::{
    BookingCoachDisplayNameLookup, BookingDetail, BookingRepository, StudentDisplayNameLookup,
};
use crate::Unavailable;

/// 本部が見る予約 1 件の詳細のユースケース（admin-web の `getBooking`。本部だけが呼ぶ）。
///
/// 予約（状態を問わない）と予約したスロットを読み、コーチと生徒の表示名を profile に問い合わせて添える。
/// 予約がなければ [`GetBookingError::NotFound`]。読むだけで何も変えない。
pub struct GetBooking<B, K, D> {
    bookings: B,
    coaches: K,
    students: D,
}

#[derive(Debug)]
pub enum GetBookingError {
    /// 予約がない
    NotFound,
    /// 保存先の障害（予約のスロットが保存されていないときも）
    Unavailable(Unavailable),
}

impl From<Unavailable> for GetBookingError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}

impl<B, K, D> GetBooking<B, K, D>
where
    B: BookingRepository,
    K: BookingCoachDisplayNameLookup,
    D: StudentDisplayNameLookup,
{
    pub fn new(bookings: B, coaches: K, students: D) -> Self {
        Self {
            bookings,
            coaches,
            students,
        }
    }

    pub async fn execute(&self, booking_id: BookingId) -> Result<BookingDetail, GetBookingError> {
        let booking = self
            .bookings
            .find(booking_id)
            .await?
            .ok_or(GetBookingError::NotFound)?;
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
        let student_display_name = self
            .students
            .find_student_display_names(&BTreeSet::from([booking.student_id()]))
            .await?
            .remove(&booking.student_id());
        Ok(BookingDetail {
            booking,
            slot,
            coach_display_name,
            student_display_name,
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
        catalog::{
            LessonDuration, LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription,
            LessonMenuId, LessonMenuName, Sport,
        },
        identity::AccountId,
        profile::DisplayName,
        scheduling::{Booking, BookingStatus, Slot, SlotId, SlotSeats, SlotStatus},
    };

    use super::*;
    use crate::in_memory::{InMemoryBookingRepository, InMemorySlotRepository};

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(7);
    const START: SystemTime = SystemTime::UNIX_EPOCH;
    const HOUR: Duration = Duration::from_secs(3600);

    fn name(value: &str) -> DisplayName {
        DisplayName::new(value).unwrap()
    }

    fn slot(id: u128) -> Slot {
        Slot::restore(
            SlotId::from_u128(id),
            COACH,
            Sport::Tennis,
            START + 48 * HOUR,
            START + 49 * HOUR,
            SlotStatus::Open,
            SlotSeats::default(),
        )
        .unwrap()
    }

    fn menu() -> LessonMenu {
        LessonMenu::restore(
            LessonMenuId::from_u128(9),
            COACH,
            LessonMenuContent {
                sport: Sport::Tennis,
                format: LessonFormat::Live(LessonDuration::from_minutes(60).unwrap()),
                name: LessonMenuName::new("フォアの基本").unwrap(),
                description: LessonMenuDescription::default(),
            },
            true,
            START,
        )
    }

    /// `known` のアカウントの表示名だけを知っている。
    #[derive(Clone, Default)]
    struct Names {
        known: BTreeMap<AccountId, DisplayName>,
    }

    impl BookingCoachDisplayNameLookup for Names {
        async fn find_coach_display_names(
            &self,
            coach_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            Ok(coach_ids
                .iter()
                .filter_map(|id| Some((*id, self.known.get(id)?.clone())))
                .collect())
        }
    }

    impl StudentDisplayNameLookup for Names {
        async fn find_student_display_names(
            &self,
            student_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            Ok(student_ids
                .iter()
                .filter_map(|id| Some((*id, self.known.get(id)?.clone())))
                .collect())
        }
    }

    async fn book(bookings: &InMemoryBookingRepository, slot: u128, is_trial: bool) -> Booking {
        bookings
            .reserve(SlotId::from_u128(slot), STUDENT, |slot, existing| {
                Booking::reserve(bookings.next_id(), STUDENT, is_trial, slot, existing, START)
                    .map(|booking| booking.with_lesson_menu(slot, Some(&menu())))
            })
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn returns_the_booking_its_slot_and_display_names() {
        let with_menu = slot(1)
            .with_lesson_menu(&menu())
            .expect("枠に結びつけられるメニュー");
        let bookings =
            InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([with_menu]));
        let booked = book(&bookings, 1, true).await;
        bookings
            .update(booked.id(), |booking, slot| {
                booking.confirm(STUDENT, slot, START + HOUR)
            })
            .await
            .unwrap();
        bookings
            .update(booked.id(), |booking, slot| {
                booking.cancel(STUDENT, slot, START + 2 * HOUR).map(|_| ())
            })
            .await
            .unwrap();
        let names = Names {
            known: BTreeMap::from([(COACH, name("たなか")), (STUDENT, name("あおき"))]),
        };

        let detail = GetBooking::new(bookings, names.clone(), names)
            .execute(booked.id())
            .await
            .unwrap();
        assert_eq!(detail.booking.id(), booked.id());
        assert!(detail.booking.is_trial());
        assert_eq!(detail.booking.status(), BookingStatus::Cancelled);
        assert_eq!(
            detail
                .booking
                .lesson_menu()
                .map(|menu| menu.name().as_str()),
            Some("フォアの基本")
        );
        assert_eq!(detail.slot.id(), SlotId::from_u128(1));
        assert_eq!(
            (detail.slot.starts_at(), detail.slot.ends_at()),
            (START + 48 * HOUR, START + 49 * HOUR)
        );
        assert_eq!(detail.coach_display_name, Some(name("たなか")));
        assert_eq!(detail.student_display_name, Some(name("あおき")));
        assert_eq!(
            detail
                .booking
                .status_changes()
                .iter()
                .map(|change| (change.status(), change.changed_at()))
                .collect::<Vec<_>>(),
            [
                (BookingStatus::Confirmed, START + HOUR),
                (BookingStatus::Cancelled, START + 2 * HOUR),
            ]
        );
    }

    #[tokio::test]
    async fn display_names_that_cannot_be_found_are_left_out() {
        let bookings =
            InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([slot(1)]));
        let booked = book(&bookings, 1, false).await;

        let detail = GetBooking::new(bookings, Names::default(), Names::default())
            .execute(booked.id())
            .await
            .unwrap();
        assert_eq!(detail.booking.status(), BookingStatus::Pending);
        assert!(detail.booking.lesson_menu().is_none(), "枠にメニューがない");
        assert!(detail.booking.status_changes().is_empty());
        assert_eq!(detail.coach_display_name, None);
        assert_eq!(detail.student_display_name, None);
    }

    #[tokio::test]
    async fn unknown_booking_is_not_found() {
        let bookings =
            InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([slot(1)]));
        book(&bookings, 1, false).await;

        let error = GetBooking::new(bookings, Names::default(), Names::default())
            .execute(BookingId::from_u128(99))
            .await
            .unwrap_err();
        assert!(matches!(error, GetBookingError::NotFound), "{error:?}");
    }
}
