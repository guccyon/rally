use std::time::SystemTime;

use super::{Booking, BookingStatus};
use crate::{identity::AccountId, lesson::RecentLessonPeriod};

/// コーチ 1 人の直近にキャンセルされた予約の数（admin-web の `getCoachOverview` の `recent_cancelled_booking_count`。期間の
/// 正本は `specs/contracts/api/admin-web/openapi.yaml` の `getCoachOverview`）。
///
/// そのコーチのスロットへの予約のうち、今の状態がキャンセル済みで、キャンセルした時刻（[`Booking::cancelled_at`]）が照会した
/// 時刻の 30 日前（含む）から照会した時刻（含む）までのものを数える。期間の境界は `listCoaches` の `recent_lesson_count` と
/// 同じ（[`RecentLessonPeriod::since`]）。誰がキャンセルしたか（[`Booking::cancelled_by`]）では分けない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachRecentCancelledBookingCount {
    coach_id: AccountId,
    cancelled_count: u32,
}

impl CoachRecentCancelledBookingCount {
    /// `booking` がコーチ `coach_id` の予約で、`now` に照会したときの期間の中にキャンセルされたか。
    pub fn counts(coach_id: AccountId, booking: &Booking, now: SystemTime) -> bool {
        booking.coach_id() == coach_id
            && booking.status() == BookingStatus::Cancelled
            && booking
                .cancelled_at()
                .is_some_and(|at| (RecentLessonPeriod::since(now)..=now).contains(&at))
    }

    /// `bookings` のうち、コーチ `coach_id` の予約で `now` に照会したときの期間の中にキャンセルされたものを数える（他の
    /// コーチの予約・キャンセルされていない予約・期間の外にキャンセルされた予約は数えない）。
    pub fn tally<'a>(
        coach_id: AccountId,
        bookings: impl IntoIterator<Item = &'a Booking>,
        now: SystemTime,
    ) -> Self {
        let n = bookings
            .into_iter()
            .filter(|booking| Self::counts(coach_id, booking, now))
            .count();
        Self {
            coach_id,
            cancelled_count: u32::try_from(n).unwrap_or(u32::MAX),
        }
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    /// 期間の中にキャンセルされた予約の数。
    pub const fn cancelled_count(&self) -> u32 {
        self.cancelled_count
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        catalog::Sport,
        scheduling::{BookingId, Slot, SlotId, SlotSeats, SlotStatus},
    };

    const DAY: Duration = Duration::from_secs(24 * 3600);
    const SECOND: Duration = Duration::from_secs(1);
    const COACH: AccountId = AccountId::from_u128(1);
    const OTHER_COACH: AccountId = AccountId::from_u128(2);
    const STUDENT: AccountId = AccountId::from_u128(3);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    fn booking(
        id: u128,
        coach_id: AccountId,
        status: BookingStatus,
        cancelled_at: Option<SystemTime>,
    ) -> Booking {
        let slot = Slot::restore(
            SlotId::from_u128(id),
            coach_id,
            Sport::Tennis,
            now() + DAY,
            now() + DAY + Duration::from_secs(3600),
            SlotStatus::Open,
            SlotSeats::default(),
        )
        .unwrap();
        Booking::restore(
            BookingId::from_u128(id),
            STUDENT,
            &slot,
            false,
            status,
            now() - 40 * DAY,
            None,
            cancelled_at,
        )
    }

    fn cancelled(id: u128, coach_id: AccountId, at: SystemTime) -> Booking {
        booking(id, coach_id, BookingStatus::Cancelled, Some(at))
    }

    #[test]
    fn counts_both_ends_of_the_window_of_30_days() {
        for (at, expected) in [
            (now(), true),
            (now() - 30 * DAY, true),
            (now() - 30 * DAY - SECOND, false),
            (now() + SECOND, false),
        ] {
            assert_eq!(
                CoachRecentCancelledBookingCount::counts(COACH, &cancelled(1, COACH, at), now()),
                expected,
                "{at:?}"
            );
        }
    }

    #[test]
    fn the_window_is_the_same_as_recent_lesson_count() {
        let since = RecentLessonPeriod::since(now());
        assert!(CoachRecentCancelledBookingCount::counts(
            COACH,
            &cancelled(1, COACH, since),
            now()
        ));
        assert!(!CoachRecentCancelledBookingCount::counts(
            COACH,
            &cancelled(1, COACH, since - SECOND),
            now()
        ));
    }

    #[test]
    fn tallies_only_cancelled_bookings_of_the_coach() {
        let bookings = [
            cancelled(1, COACH, now() - DAY),
            cancelled(2, COACH, now() - 29 * DAY),
            cancelled(3, COACH, now() - 31 * DAY),
            cancelled(4, OTHER_COACH, now() - DAY),
            booking(5, COACH, BookingStatus::Pending, None),
            booking(6, COACH, BookingStatus::Confirmed, None),
        ];
        let count = CoachRecentCancelledBookingCount::tally(COACH, &bookings, now());
        assert_eq!(count.coach_id(), COACH);
        assert_eq!(
            count.cancelled_count(),
            2,
            "31 日前のキャンセル・他のコーチの予約・有効な予約は数えない"
        );

        let empty = CoachRecentCancelledBookingCount::tally(COACH, &[], now());
        assert_eq!(empty.cancelled_count(), 0);
    }
}
