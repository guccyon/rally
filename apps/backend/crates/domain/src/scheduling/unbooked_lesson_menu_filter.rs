/// 本部がレッスンメニューを「直近に予約のないメニュー」に絞るときの条件（admin-web の `listLessonMenus` の `unbooked`）。
/// 「直近に予約のない」をどう決めるかはここだけに置く。
///
/// - `unbooked` が `true`: 直近の期間に数える予約の数（[`super::RecentLessonMenuBookings::tally`] で数えたもの。
///   `listLessonMenus` の `recent_booking_count` と同じ数）が 0 のメニューだけ
/// - `false`（既定）: 絞らない（どのメニューも通す）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnbookedLessonMenuFilter(bool);

impl UnbookedLessonMenuFilter {
    /// `unbooked` が `true` なら直近に予約のないメニューだけに絞る条件。`false` なら絞らない。
    pub const fn new(unbooked: bool) -> Self {
        Self(unbooked)
    }

    /// 直近に予約のないメニューだけに絞るか。
    pub const fn unbooked(&self) -> bool {
        self.0
    }

    /// 直近の予約の数が `recent_booking_count` のメニューがこの条件に合うか。
    /// [`super::RecentLessonMenuBookings::tally`] の結果にないメニューは 0 として渡す。
    pub const fn matches(&self, recent_booking_count: u32) -> bool {
        !self.0 || recent_booking_count == 0
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::{
        catalog::{LessonMenuId, LessonMenuName, Sport},
        identity::AccountId,
        scheduling::{
            Booking, BookingId, BookingLessonMenu, BookingStatus, RecentLessonMenuBookings, Slot,
            SlotId, SlotSeats, SlotStatus,
        },
    };

    const MENU: LessonMenuId = LessonMenuId::from_u128(5);
    const OTHER_MENU: LessonMenuId = LessonMenuId::from_u128(6);
    const DAY: Duration = Duration::from_secs(24 * 3600);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    fn booking(
        id: u128,
        menu: LessonMenuId,
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
        .with_restored_lesson_menu(Some(BookingLessonMenu::restore(
            menu,
            LessonMenuName::new("フォア改善").unwrap(),
            None,
        )))
    }

    /// `bookings` を直近の予約として数えたときの [`MENU`] の数（結果になければ 0）。
    fn recent(bookings: &[Booking]) -> u32 {
        RecentLessonMenuBookings::tally(bookings, now())
            .get(&MENU)
            .copied()
            .unwrap_or(0)
    }

    #[test]
    fn unbooked_matches_only_menus_without_bookings_in_the_recent_period() {
        let filter = UnbookedLessonMenuFilter::new(true);
        assert!(filter.unbooked());
        assert!(filter.matches(recent(&[])), "予約がなければ載る");
        assert!(
            filter.matches(recent(&[
                // 30 日より前の予約・キャンセルした予約・他のメニューの予約は数えない
                booking(
                    1,
                    MENU,
                    BookingStatus::Confirmed,
                    now() - 30 * DAY - Duration::from_secs(1),
                ),
                booking(2, MENU, BookingStatus::Cancelled, now() - DAY),
                booking(3, OTHER_MENU, BookingStatus::Confirmed, now()),
            ])),
            "数えない予約だけなら載る"
        );
        for status in [BookingStatus::Pending, BookingStatus::Confirmed] {
            assert!(
                !filter.matches(recent(&[booking(1, MENU, status, now() - 30 * DAY)])),
                "30 日前ちょうどの未確定・確定の予約は数えるので載らない: {status:?}"
            );
        }
    }

    #[test]
    fn not_unbooked_matches_every_menu() {
        let filter = UnbookedLessonMenuFilter::default();
        assert_eq!(filter, UnbookedLessonMenuFilter::new(false));
        assert!(!filter.unbooked());
        assert!(filter.matches(recent(&[])));
        assert!(filter.matches(recent(&[booking(1, MENU, BookingStatus::Confirmed, now())])));
    }
}
