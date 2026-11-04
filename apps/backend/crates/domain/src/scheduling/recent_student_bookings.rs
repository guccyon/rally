use std::{collections::BTreeMap, time::SystemTime};

use super::{Booking, RecentLessonMenuBookings};
use crate::identity::AccountId;

/// 本部が見る生徒ごとの直近の予約の数（admin-web の `listStudents` の `recent_booking_count`。期間の正本は
/// `specs/contracts/api/admin-web/openapi.yaml` の `listStudents`）の数え方。
///
/// 期間とキャンセルの扱いはレッスンメニューごとの数（[`RecentLessonMenuBookings`]）に揃える: 照会した時刻の
/// [`RecentLessonMenuBookings::WINDOW_DAYS`] 日前（含む）から照会した時刻（含む）までに作られた予約（[`Booking::created_at`]）
/// のうち、キャンセルされていないもの（今の状態が未確定か確定）を、予約した生徒（[`Booking::student_id`]）ごとに数える。
/// メニューごとの数と違い、メニューの写しのない予約も数える（生徒ごとに数えるので、どのメニューかは問わない）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecentStudentBookings;

impl RecentStudentBookings {
    /// `now` に照会したときの期間の始まり（含む）。期間は [`RecentLessonMenuBookings::since`] と同じ。
    pub fn since(now: SystemTime) -> SystemTime {
        RecentLessonMenuBookings::since(now)
    }

    /// `booking` が `now` に照会したときに数える予約か（期間に作られ、キャンセルされていない）。
    pub fn counts(booking: &Booking, now: SystemTime) -> bool {
        booking.status().is_active() && (Self::since(now)..=now).contains(&booking.created_at())
    }

    /// `bookings` のうち `now` に照会したときに数える予約を、生徒の ID ごとに数える。数える予約が 1 件もない生徒は
    /// 結果に含めない。
    pub fn tally<'a>(
        bookings: impl IntoIterator<Item = &'a Booking>,
        now: SystemTime,
    ) -> BTreeMap<AccountId, u32> {
        let mut counts = BTreeMap::new();
        for booking in bookings {
            if Self::counts(booking, now) {
                let count: &mut u32 = counts.entry(booking.student_id()).or_default();
                *count = count.saturating_add(1);
            }
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        catalog::{LessonMenuId, LessonMenuName, Sport},
        scheduling::{
            BookingId, BookingLessonMenu, BookingStatus, Slot, SlotId, SlotSeats, SlotStatus,
        },
    };

    const DAY: Duration = Duration::from_secs(24 * 3600);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    fn booking(
        id: u128,
        student: u128,
        menu: Option<u128>,
        status: BookingStatus,
        created_at: SystemTime,
    ) -> Booking {
        let slot = Slot::restore(
            SlotId::from_u128(id),
            AccountId::from_u128(7),
            Sport::Tennis,
            now() + DAY,
            now() + DAY + Duration::from_secs(3600),
            SlotStatus::Open,
            SlotSeats::default(),
        )
        .unwrap();
        let (confirmed_at, cancelled_at) = match status {
            BookingStatus::Pending => (None, None),
            BookingStatus::Confirmed => (Some(created_at), None),
            BookingStatus::Cancelled => (None, Some(created_at)),
        };
        Booking::restore(
            BookingId::from_u128(id),
            AccountId::from_u128(student),
            &slot,
            false,
            status,
            created_at,
            confirmed_at,
            cancelled_at,
        )
        .with_restored_lesson_menu(menu.map(|menu| {
            BookingLessonMenu::restore(
                LessonMenuId::from_u128(menu),
                LessonMenuName::new("フォア改善").unwrap(),
                None,
            )
        }))
    }

    #[test]
    fn the_window_is_the_same_as_the_lesson_menu_count() {
        assert_eq!(RecentStudentBookings::since(now()), now() - 30 * DAY);
        assert_eq!(
            RecentStudentBookings::since(now()),
            RecentLessonMenuBookings::since(now())
        );
    }

    #[test]
    fn counts_bookings_created_within_the_last_thirty_days_including_both_ends() {
        let since = now() - 30 * DAY;
        for (created_at, counted, why) in [
            (since, true, "30 日前ちょうどは含む"),
            (
                since - Duration::from_secs(1),
                false,
                "30 日前より前は含まない",
            ),
            (now(), true, "照会した時刻ちょうどは含む"),
            (
                now() + Duration::from_secs(1),
                false,
                "照会した時刻より後は含まない",
            ),
        ] {
            assert_eq!(
                RecentStudentBookings::counts(
                    &booking(1, 101, Some(5), BookingStatus::Pending, created_at),
                    now()
                ),
                counted,
                "{why}"
            );
        }
    }

    #[test]
    fn cancelled_bookings_are_not_counted_but_bookings_without_a_menu_are() {
        for (status, menu, counted) in [
            (BookingStatus::Pending, Some(5), true),
            (BookingStatus::Confirmed, Some(5), true),
            (BookingStatus::Cancelled, Some(5), false),
            (BookingStatus::Confirmed, None, true),
            (BookingStatus::Cancelled, None, false),
        ] {
            assert_eq!(
                RecentStudentBookings::counts(&booking(1, 101, menu, status, now() - DAY), now()),
                counted,
                "{status:?} {menu:?}"
            );
        }
    }

    #[test]
    fn tally_counts_per_student_and_leaves_out_students_without_counted_bookings() {
        let bookings = [
            booking(1, 101, Some(5), BookingStatus::Pending, now() - DAY),
            booking(2, 101, None, BookingStatus::Confirmed, now() - 10 * DAY),
            booking(3, 102, Some(6), BookingStatus::Confirmed, now()),
            // 数えない（キャンセル・期間の外）
            booking(4, 101, Some(5), BookingStatus::Cancelled, now() - DAY),
            booking(5, 103, Some(7), BookingStatus::Confirmed, now() - 31 * DAY),
        ];
        assert_eq!(
            RecentStudentBookings::tally(&bookings, now()),
            BTreeMap::from([
                (AccountId::from_u128(101), 2),
                (AccountId::from_u128(102), 1)
            ]),
        );
    }
}
