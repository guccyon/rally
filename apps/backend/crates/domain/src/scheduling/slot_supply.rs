use std::{collections::HashSet, time::Duration};

use super::{Booking, Slot, SlotStatus, SlotSupplyStatus};
use crate::identity::Sport;

/// スポーツ 1 つの予約枠の需給（admin-web の `getSlotSupply`。式と期間の正本は
/// `specs/contracts/api/admin-web/openapi.yaml` の `getSlotSupply`）。
///
/// 期間（[`SlotSupply::WINDOW`]）に開始する枠について、次の 2 つを数えて比べる。
///
/// - 公開した枠の数（`published_slots`）: そのスポーツの枠のうち、公開中（`open`）か予約で埋まっている（`booked`）もの。
///   コーチが閉じた枠（`closed`）は数えない
/// - 予約しようとした生徒の数（`booking_students`）: そのスポーツの枠に予約を作った生徒の数（同じ生徒は 1 人。
///   確定・キャンセル・未確定を問わない。閉じた枠への予約も数える）
///
/// 予約しようとした生徒が 1 人以上いて、公開した枠 1 つあたり [`SlotSupply::SHORTAGE_PERCENT`] % 以上いれば
/// 「足りていない」、それ以外は「足りている」。検索は記録していないので数に入らない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotSupply {
    sport: Sport,
    published_slots: u32,
    booking_students: u32,
}

impl SlotSupply {
    /// 数える期間の長さ。集計した時刻の 30 日前（含む）から集計した時刻（含まない）までに開始する枠を数える
    pub const WINDOW: Duration = Duration::from_secs(30 * 24 * 3600);

    /// 公開した枠に対する、予約しようとした生徒の数の割合（%）がこれ以上なら「足りていない」。
    /// 60 は仮の初期値（合成シードで見て決めた。実データで見直す。契約の `getSlotSupply` の説明と同じ）
    pub const SHORTAGE_PERCENT: u32 = 60;

    pub const fn new(sport: Sport, published_slots: u32, booking_students: u32) -> Self {
        Self {
            sport,
            published_slots,
            booking_students,
        }
    }

    /// `slots`（期間に開始する枠）と `bookings`（それらの枠への予約）から `sport` の需給を数える。
    /// `slots` にない枠への予約は数えない。
    pub fn tally(sport: Sport, slots: &[Slot], bookings: &[Booking]) -> Self {
        let of_sport: Vec<&Slot> = slots.iter().filter(|s| s.sport() == sport).collect();
        let published_slots = of_sport
            .iter()
            .filter(|s| matches!(s.status(), SlotStatus::Open | SlotStatus::Booked))
            .count();
        let slot_ids: HashSet<_> = of_sport.iter().map(|s| s.id()).collect();
        let booking_students = bookings
            .iter()
            .filter(|b| slot_ids.contains(&b.slot_id()))
            .map(Booking::student_id)
            .collect::<HashSet<_>>()
            .len();
        Self::new(
            sport,
            u32::try_from(published_slots).unwrap_or(u32::MAX),
            u32::try_from(booking_students).unwrap_or(u32::MAX),
        )
    }

    pub const fn sport(&self) -> Sport {
        self.sport
    }

    pub const fn published_slots(&self) -> u32 {
        self.published_slots
    }

    pub const fn booking_students(&self) -> u32 {
        self.booking_students
    }

    /// 足りているか。予約しようとした生徒がいなければ（枠がなくても）足りている。
    pub const fn status(&self) -> SlotSupplyStatus {
        let students = self.booking_students as u64;
        let slots = self.published_slots as u64;
        if students > 0 && students * 100 >= slots * Self::SHORTAGE_PERCENT as u64 {
            SlotSupplyStatus::Insufficient
        } else {
            SlotSupplyStatus::Sufficient
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;
    use crate::{
        identity::AccountId,
        scheduling::{BookingId, SlotId},
    };

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(hour * 3600)
    }

    fn slot(id: u128, sport: Sport, status: SlotStatus) -> Slot {
        Slot::restore(
            SlotId::from_u128(id),
            AccountId::from_u128(100),
            sport,
            at(id as u64),
            at(id as u64 + 1),
            status,
            crate::scheduling::SlotSeats::default(),
        )
        .unwrap()
    }

    fn booking(id: u128, student: u128, slot: &Slot) -> Booking {
        let mut open = Slot::restore(
            slot.id(),
            slot.coach_id(),
            slot.sport(),
            slot.starts_at(),
            slot.ends_at(),
            SlotStatus::Open,
            crate::scheduling::SlotSeats::default(),
        )
        .unwrap();
        Booking::reserve(
            BookingId::from_u128(id),
            AccountId::from_u128(student),
            false,
            &mut open,
            &[],
            at(0),
        )
        .unwrap()
    }

    #[test]
    fn the_window_is_thirty_days_and_the_threshold_is_sixty_percent() {
        assert_eq!(SlotSupply::WINDOW, Duration::from_secs(2_592_000));
        assert_eq!(SlotSupply::SHORTAGE_PERCENT, 60);
    }

    #[test]
    fn insufficient_from_sixty_percent_of_the_published_slots() {
        let status = |slots, students| SlotSupply::new(Sport::Tennis, slots, students).status();
        // 10 枠に 6 人（60%）から足りていない
        assert_eq!(status(10, 5), SlotSupplyStatus::Sufficient);
        assert_eq!(status(10, 6), SlotSupplyStatus::Insufficient);
        assert_eq!(status(10, 7), SlotSupplyStatus::Insufficient);
        // 端数: 3 / 5 = 60% は足りていない、2 / 4 = 50% は足りている、5 / 8 = 62.5% は足りていない
        assert_eq!(status(5, 3), SlotSupplyStatus::Insufficient);
        assert_eq!(status(4, 2), SlotSupplyStatus::Sufficient);
        assert_eq!(status(8, 5), SlotSupplyStatus::Insufficient);
        assert_eq!(status(167, 100), SlotSupplyStatus::Sufficient);
        assert_eq!(status(166, 100), SlotSupplyStatus::Insufficient);
    }

    #[test]
    fn no_students_is_sufficient_and_students_without_slots_is_not() {
        let status = |slots, students| SlotSupply::new(Sport::Tennis, slots, students).status();
        assert_eq!(status(0, 0), SlotSupplyStatus::Sufficient);
        assert_eq!(status(3, 0), SlotSupplyStatus::Sufficient);
        assert_eq!(status(0, 1), SlotSupplyStatus::Insufficient);
    }

    #[test]
    fn large_counts_do_not_overflow() {
        let status = |slots, students| SlotSupply::new(Sport::Tennis, slots, students).status();
        assert_eq!(status(u32::MAX, u32::MAX), SlotSupplyStatus::Insufficient);
        assert_eq!(status(u32::MAX, 1), SlotSupplyStatus::Sufficient);
    }

    #[test]
    fn tally_counts_open_and_booked_slots_of_the_sport_but_not_closed_ones() {
        let slots = [
            slot(1, Sport::Tennis, SlotStatus::Open),
            slot(2, Sport::Tennis, SlotStatus::Booked),
            slot(3, Sport::Tennis, SlotStatus::Closed),
            slot(4, Sport::Soccer, SlotStatus::Open),
        ];
        let supply = SlotSupply::tally(Sport::Tennis, &slots, &[]);
        assert_eq!(supply, SlotSupply::new(Sport::Tennis, 2, 0));
        assert_eq!(supply.sport(), Sport::Tennis);
        assert_eq!(supply.published_slots(), 2);
        assert_eq!(supply.booking_students(), 0);
        assert_eq!(
            SlotSupply::tally(Sport::Golf, &slots, &[]),
            SlotSupply::new(Sport::Golf, 0, 0)
        );
    }

    #[test]
    fn tally_counts_each_student_who_booked_a_slot_of_the_sport_once() {
        let slots = [
            slot(1, Sport::Tennis, SlotStatus::Open),
            slot(2, Sport::Tennis, SlotStatus::Booked),
            slot(3, Sport::Tennis, SlotStatus::Closed),
            slot(4, Sport::Soccer, SlotStatus::Open),
        ];
        let mut cancelled = booking(10, 7, &slots[0]);
        cancelled
            .cancel(AccountId::from_u128(7), &mut slots[0].clone(), at(0))
            .unwrap();
        let bookings = [
            // 同じ生徒の 2 件目は数えない。キャンセルした予約も数える
            cancelled,
            booking(11, 7, &slots[1]),
            booking(12, 8, &slots[1]),
            // 閉じた枠への予約も、予約しようとした生徒に入る
            booking(13, 9, &slots[2]),
            // 別のスポーツの枠
            booking(14, 10, &slots[3]),
            // 期間の枠にない枠への予約
            booking(15, 11, &slot(99, Sport::Tennis, SlotStatus::Open)),
        ];
        assert_eq!(
            SlotSupply::tally(Sport::Tennis, &slots, &bookings),
            SlotSupply::new(Sport::Tennis, 2, 3)
        );
        assert_eq!(
            SlotSupply::tally(Sport::Soccer, &slots, &bookings),
            SlotSupply::new(Sport::Soccer, 1, 1)
        );
    }
}
