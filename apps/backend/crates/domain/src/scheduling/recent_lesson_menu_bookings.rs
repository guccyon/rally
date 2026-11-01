use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime},
};

use super::Booking;
use crate::catalog::LessonMenuId;

/// 本部が見るレッスンメニューごとの直近の予約の数（admin-web の `listLessonMenus` の `recent_booking_count`。期間の正本は
/// `specs/contracts/api/admin-web/openapi.yaml` の `listLessonMenus`）の数え方。
///
/// 照会した時刻の [`RecentLessonMenuBookings::WINDOW_DAYS`] 日前（含む）から照会した時刻（含む）までに作られた予約
/// （予約を作った時刻 [`Booking::created_at`]）のうち、キャンセルされていないもの（今の状態が未確定か確定）を、予約した時点の
/// レッスンメニューの写し（[`Booking::lesson_menu`]）の ID ごとに数える。メニューの写しのない予約は数えない。
/// 期間と数える予約の規則はこの型の 1 か所だけに置く。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecentLessonMenuBookings;

impl RecentLessonMenuBookings {
    /// 数える期間の日数（直近 30 日）
    pub const WINDOW_DAYS: u32 = 30;
    /// 数える期間の長さ（[`Self::WINDOW_DAYS`] 日）
    pub const WINDOW: Duration = Duration::from_secs(Self::WINDOW_DAYS as u64 * 24 * 3600);

    /// `now` に照会したときの期間の始まり（含む）。`now` から期間を引けない（時刻の範囲の外）ときは UNIX 時刻の起点。
    pub fn since(now: SystemTime) -> SystemTime {
        now.checked_sub(Self::WINDOW)
            .unwrap_or(SystemTime::UNIX_EPOCH)
    }

    /// `booking` が `now` に照会したときに数える予約か（期間に作られ、キャンセルされておらず、メニューの写しがある）。
    pub fn counts(booking: &Booking, now: SystemTime) -> bool {
        booking.status().is_active()
            && booking.lesson_menu().is_some()
            && (Self::since(now)..=now).contains(&booking.created_at())
    }

    /// `bookings` のうち `now` に照会したときに数える予約を、メニューの ID ごとに数える。数える予約が 1 件もない
    /// メニューは結果に含めない。
    pub fn tally<'a>(
        bookings: impl IntoIterator<Item = &'a Booking>,
        now: SystemTime,
    ) -> BTreeMap<LessonMenuId, u32> {
        let mut counts = BTreeMap::new();
        for booking in bookings {
            if !Self::counts(booking, now) {
                continue;
            }
            if let Some(menu) = booking.lesson_menu() {
                let count: &mut u32 = counts.entry(menu.id()).or_default();
                *count = count.saturating_add(1);
            }
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        catalog::{LessonMenuName, Sport},
        identity::AccountId,
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
            AccountId::from_u128(100 + id),
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
    fn the_window_is_thirty_days() {
        assert_eq!(RecentLessonMenuBookings::WINDOW_DAYS, 30);
        assert_eq!(RecentLessonMenuBookings::WINDOW, 30 * DAY);
        assert_eq!(RecentLessonMenuBookings::since(now()), now() - 30 * DAY);
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
                RecentLessonMenuBookings::counts(
                    &booking(1, Some(5), BookingStatus::Pending, created_at),
                    now()
                ),
                counted,
                "{why}"
            );
        }
    }

    #[test]
    fn cancelled_bookings_and_bookings_without_a_menu_are_not_counted() {
        for (status, menu, counted) in [
            (BookingStatus::Pending, Some(5), true),
            (BookingStatus::Confirmed, Some(5), true),
            (BookingStatus::Cancelled, Some(5), false),
            (BookingStatus::Confirmed, None, false),
        ] {
            assert_eq!(
                RecentLessonMenuBookings::counts(&booking(1, menu, status, now() - DAY), now()),
                counted,
                "{status:?} {menu:?}"
            );
        }
    }

    #[test]
    fn tally_counts_per_menu_and_leaves_out_menus_without_counted_bookings() {
        let bookings = [
            booking(1, Some(5), BookingStatus::Pending, now() - DAY),
            booking(2, Some(5), BookingStatus::Confirmed, now() - 10 * DAY),
            booking(3, Some(6), BookingStatus::Confirmed, now()),
            // 数えない（キャンセル・期間の外・メニューなし）
            booking(4, Some(5), BookingStatus::Cancelled, now() - DAY),
            booking(5, Some(7), BookingStatus::Confirmed, now() - 31 * DAY),
            booking(6, None, BookingStatus::Confirmed, now() - DAY),
        ];
        assert_eq!(
            RecentLessonMenuBookings::tally(&bookings, now()),
            BTreeMap::from([
                (LessonMenuId::from_u128(5), 2),
                (LessonMenuId::from_u128(6), 1)
            ]),
        );
    }
}
