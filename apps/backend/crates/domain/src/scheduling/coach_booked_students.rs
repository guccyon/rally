use std::collections::BTreeSet;

use super::Booking;
use crate::identity::AccountId;

/// 本部が見る生徒の一覧（admin-web の `listStudents` の `coach_id`）で、生徒が「そのコーチを予約したことがある」かの数え方。
///
/// そのコーチの枠（[`Booking::coach_id`]）への予約が 1 件でもあれば、予約したことがある生徒とする。予約の状態（確定待ち・
/// 確定済み・キャンセル済み）は問わない。本部が見る生徒 1 人の利用の様子（`getStudentActivity`）がキャンセル済みの予約も
/// 生徒の予約として数え（`cancelled_booking_count`）、生徒の一覧の最後に予約した時刻も予約の状態を問わないのに揃える
/// （あとでキャンセルしても予約した事実は残る）。体験予約も予約の 1 つとして数える。数え方はこの型の 1 か所だけに置く。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoachBookedStudents;

impl CoachBookedStudents {
    /// `booking` が、`coach_id` のコーチを予約したことがある数えに入る予約か（そのコーチの枠への予約。状態は問わない）。
    pub fn counts(booking: &Booking, coach_id: AccountId) -> bool {
        booking.coach_id() == coach_id
    }

    /// `bookings` のうち数えに入る予約をした生徒の ID。数えに入る予約がなければ空。
    pub fn collect<'a>(
        bookings: impl IntoIterator<Item = &'a Booking>,
        coach_id: AccountId,
    ) -> BTreeSet<AccountId> {
        bookings
            .into_iter()
            .filter(|booking| Self::counts(booking, coach_id))
            .map(Booking::student_id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::{
        catalog::Sport,
        scheduling::{BookingId, BookingStatus, Slot, SlotId, SlotSeats, SlotStatus},
    };

    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800 + hour * 3600)
    }

    /// ID が偶数の予約は体験予約。
    fn booking(id: u128, student: u128, coach: AccountId, status: BookingStatus) -> Booking {
        let slot = Slot::restore(
            SlotId::from_u128(id),
            coach,
            Sport::Tennis,
            at(24),
            at(25),
            SlotStatus::Open,
            SlotSeats::default(),
        )
        .unwrap();
        let (confirmed_at, cancelled_at) = match status {
            BookingStatus::Pending => (None, None),
            BookingStatus::Confirmed => (Some(at(1)), None),
            BookingStatus::Cancelled => (None, Some(at(1))),
        };
        Booking::restore(
            BookingId::from_u128(id),
            AccountId::from_u128(student),
            &slot,
            id.is_multiple_of(2),
            status,
            at(0),
            confirmed_at,
            cancelled_at,
        )
    }

    #[test]
    fn bookings_of_the_coach_count_whatever_the_status() {
        for status in BookingStatus::ALL {
            assert!(
                CoachBookedStudents::counts(&booking(1, 100, COACH, status), COACH),
                "{status:?}"
            );
            assert!(
                !CoachBookedStudents::counts(&booking(1, 100, OTHER_COACH, status), COACH),
                "ほかのコーチの枠への予約は数えない: {status:?}"
            );
        }
    }

    #[test]
    fn collects_each_student_once_including_cancelled_and_trial_bookings() {
        let bookings = [
            booking(1, 100, COACH, BookingStatus::Confirmed),
            // 同じ生徒の 2 件目は 1 人として数える
            booking(2, 100, COACH, BookingStatus::Pending),
            // キャンセル済みでも予約したことがある（体験予約も）
            booking(3, 101, COACH, BookingStatus::Cancelled),
            booking(4, 102, COACH, BookingStatus::Cancelled),
            // ほかのコーチだけを予約した生徒は入らない
            booking(5, 103, OTHER_COACH, BookingStatus::Confirmed),
        ];
        assert_eq!(
            CoachBookedStudents::collect(&bookings, COACH),
            [100, 101, 102].map(AccountId::from_u128).into()
        );
        assert!(CoachBookedStudents::collect(&bookings, AccountId::from_u128(9)).is_empty());
        assert!(CoachBookedStudents::collect(&[], COACH).is_empty());
    }
}
