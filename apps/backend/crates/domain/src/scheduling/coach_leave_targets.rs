use std::collections::BTreeSet;

use super::{Booking, BookingId, BookingPeriod, Slot, SlotStatus};
use crate::identity::AccountId;

/// コーチのお休みで片づけるもの（[`super::CoachLeave`]）。本部が見る期間の予約と枠の確かめ（admin-web の
/// `getCoachLeavePreview`）と、お休みにする操作（[`super::CoachLeave::start`]）が、同じここで対象を決める（見ている一覧と
/// 片づけるものをいつも一致させる）。
///
/// 対象はコーチの枠のうち開始が期間に入るもの（[`BookingPeriod::includes`]。終わりが期間の外にはみ出してもよい）と、
/// それらの枠への有効な予約（確定待ち・確定済み）。キャンセル済みの予約は含めない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachLeaveTargets {
    bookings: Vec<(Booking, Slot)>,
    open_slots: Vec<Slot>,
    slot_count: usize,
}

impl CoachLeaveTargets {
    /// `coach_id` のコーチの期間 `period` の対象を、`slots`（少なくともそのコーチの、開始が期間に入る枠）と `bookings`
    /// （少なくともそれらの枠への予約）から選ぶ。ほかのコーチの枠・期間の外の枠は無視する。予約は枠の開始の早い順（同じ
    /// 開始なら予約の ID の順）、予約のない枠も開始の早い順に並べる。予約の枠が `slots` になければ、その予約は含めない。
    pub fn of(
        coach_id: AccountId,
        period: BookingPeriod,
        slots: &[Slot],
        bookings: &[Booking],
    ) -> Self {
        let mut in_period: Vec<&Slot> = slots
            .iter()
            .filter(|slot| slot.coach_id() == coach_id && period.includes(slot.starts_at()))
            .collect();
        in_period.sort_by_key(|slot| (slot.starts_at(), slot.id()));
        let mut active: Vec<(Booking, Slot)> = bookings
            .iter()
            .filter(|booking| booking.status().is_active())
            .filter_map(|booking| {
                in_period
                    .iter()
                    .find(|slot| slot.id() == booking.slot_id())
                    .map(|slot| (booking.clone(), (*slot).clone()))
            })
            .collect();
        active.sort_by_key(|(booking, _)| (booking.starts_at(), booking.id()));
        let booked: BTreeSet<_> = active
            .iter()
            .map(|(booking, _)| booking.slot_id())
            .collect();
        let open_slots = in_period
            .iter()
            .filter(|slot| slot.status() == SlotStatus::Open && !booked.contains(&slot.id()))
            .map(|slot| (*slot).clone())
            .collect();
        let slot_count = in_period
            .iter()
            .filter(|slot| slot.status() != SlotStatus::Closed)
            .count();
        Self {
            bookings: active,
            open_slots,
            slot_count,
        }
    }

    /// 期間の有効な予約と、その予約した枠（並びは [`Self::of`]）。グループレッスンの枠は予約ごとに 1 件。
    pub fn bookings(&self) -> &[(Booking, Slot)] {
        &self.bookings
    }

    /// 期間の、公開中で有効な予約のない枠（並びは [`Self::of`]）。
    pub fn open_slots(&self) -> &[Slot] {
        &self.open_slots
    }

    /// 期間の有効な予約の ID（本部が確かめたものとして送り返す値）。
    pub fn booking_ids(&self) -> BTreeSet<BookingId> {
        self.bookings
            .iter()
            .map(|(booking, _)| booking.id())
            .collect()
    }

    /// 取り下げる枠の数（期間の、終了でない枠。予約のある枠を含む）。
    pub const fn slot_count(&self) -> usize {
        self.slot_count
    }

    /// 知らせる生徒の人数（期間の有効な予約をした生徒。同じ生徒は 1 人と数える）。
    pub fn student_count(&self) -> usize {
        self.bookings
            .iter()
            .map(|(booking, _)| booking.student_id())
            .collect::<BTreeSet<_>>()
            .len()
    }

    /// 片づけるものがないか（期間に有効な予約も、公開中の枠もない）。
    pub fn is_empty(&self) -> bool {
        self.bookings.is_empty() && self.open_slots.is_empty()
    }
}
