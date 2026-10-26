use std::time::SystemTime;

use super::{SlotId, SlotStatus};
use crate::{
    DomainError,
    identity::{AccountId, Sport},
};

/// スロット（glossary の slot）。コーチが公開する予約枠で、開始は終了より前。
///
/// 期間は開始を含み終了を含まない（`[starts_at, ends_at)`）。前の枠の終了と次の枠の開始が
/// 同じ時刻なら重ならない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    id: SlotId,
    coach_id: AccountId,
    sport: Sport,
    starts_at: SystemTime,
    ends_at: SystemTime,
    status: SlotStatus,
}

impl Slot {
    /// 1 つの枠に入れる予約の数。Gen1 はマンツーマンだけなので 1。
    pub const CAPACITY: u32 = 1;

    /// 公開する枠を作る（状態は [`SlotStatus::Open`]）。開始が終了より前でなければ
    /// [`DomainError::SlotEndsBeforeItStarts`]。同じコーチの枠との重なりは [`super::CoachCalendar`] が判定する。
    pub fn publish(
        id: SlotId,
        coach_id: AccountId,
        sport: Sport,
        starts_at: SystemTime,
        ends_at: SystemTime,
    ) -> Result<Self, DomainError> {
        Self::restore(id, coach_id, sport, starts_at, ends_at, SlotStatus::Open)
    }

    /// 保存した枠を読み戻す（状態を問わない）。開始と終了の検証は [`Slot::publish`] と同じ。
    pub fn restore(
        id: SlotId,
        coach_id: AccountId,
        sport: Sport,
        starts_at: SystemTime,
        ends_at: SystemTime,
        status: SlotStatus,
    ) -> Result<Self, DomainError> {
        if starts_at >= ends_at {
            return Err(DomainError::SlotEndsBeforeItStarts);
        }
        Ok(Self {
            id,
            coach_id,
            sport,
            starts_at,
            ends_at,
            status,
        })
    }

    pub const fn id(&self) -> SlotId {
        self.id
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    pub const fn sport(&self) -> Sport {
        self.sport
    }

    pub const fn starts_at(&self) -> SystemTime {
        self.starts_at
    }

    pub const fn ends_at(&self) -> SystemTime {
        self.ends_at
    }

    pub const fn status(&self) -> SlotStatus {
        self.status
    }

    pub const fn capacity(&self) -> u32 {
        Self::CAPACITY
    }

    /// 予約の確定で枠を埋める（[`SlotStatus::Open`] → [`SlotStatus::Booked`]）。公開中でなければ
    /// [`DomainError::SlotNotOpen`] で、状態は変えない。予約の確定（[`super::Booking::confirm`]）から呼ぶ。
    pub fn book(&mut self) -> Result<(), DomainError> {
        if self.status != SlotStatus::Open {
            return Err(DomainError::SlotNotOpen {
                status: self.status,
            });
        }
        self.status = SlotStatus::Booked;
        Ok(())
    }

    /// 確定した予約のキャンセルで枠を公開中に戻す（[`SlotStatus::Booked`] → [`SlotStatus::Open`]）。
    /// 予約済みでなければ [`DomainError::SlotNotBooked`] で、状態は変えない。予約のキャンセル
    /// （[`super::Booking::cancel`]）から呼ぶ。
    pub fn release(&mut self) -> Result<(), DomainError> {
        if self.status != SlotStatus::Booked {
            return Err(DomainError::SlotNotBooked {
                status: self.status,
            });
        }
        self.status = SlotStatus::Open;
        Ok(())
    }

    /// 期間が重なるか（コーチ・状態は見ない）。
    pub fn overlaps(&self, other: &Slot) -> bool {
        self.starts_at < other.ends_at && other.starts_at < self.ends_at
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(hour * 3600)
    }

    fn slot(id: u128, from: u64, to: u64) -> Slot {
        Slot::publish(
            SlotId::from_u128(id),
            AccountId::from_u128(1),
            Sport::Tennis,
            at(from),
            at(to),
        )
        .unwrap()
    }

    #[test]
    fn published_slot_is_open_with_capacity_one() {
        let slot = slot(1, 10, 11);
        assert_eq!(slot.status(), SlotStatus::Open);
        assert_eq!(slot.capacity(), 1);
        assert_eq!((slot.starts_at(), slot.ends_at()), (at(10), at(11)));
    }

    #[test]
    fn slot_must_start_before_it_ends() {
        for (from, to) in [(10, 10), (11, 10)] {
            assert_eq!(
                Slot::publish(
                    SlotId::from_u128(1),
                    AccountId::from_u128(1),
                    Sport::Tennis,
                    at(from),
                    at(to),
                ),
                Err(DomainError::SlotEndsBeforeItStarts)
            );
        }
    }

    #[test]
    fn booking_fills_only_an_open_slot() {
        let mut slot = slot(1, 10, 11);
        slot.book().unwrap();
        assert_eq!(slot.status(), SlotStatus::Booked);
        assert_eq!(
            slot.book(),
            Err(DomainError::SlotNotOpen {
                status: SlotStatus::Booked
            })
        );
        assert_eq!(slot.status(), SlotStatus::Booked);
    }

    #[test]
    fn releasing_reopens_only_a_booked_slot() {
        let mut slot = slot(1, 10, 11);
        assert_eq!(
            slot.release(),
            Err(DomainError::SlotNotBooked {
                status: SlotStatus::Open
            })
        );
        slot.book().unwrap();
        slot.release().unwrap();
        assert_eq!(slot.status(), SlotStatus::Open);
    }

    #[test]
    fn overlap_is_half_open() {
        let base = slot(1, 10, 12);
        assert!(base.overlaps(&slot(2, 11, 13)));
        assert!(base.overlaps(&slot(2, 9, 11)));
        assert!(base.overlaps(&slot(2, 10, 12)));
        assert!(base.overlaps(&slot(2, 10, 11)));
        assert!(base.overlaps(&slot(2, 9, 13)));
        // 終了と開始が同じ時刻なら重ならない
        assert!(!base.overlaps(&slot(2, 12, 13)));
        assert!(!base.overlaps(&slot(2, 8, 10)));
    }
}
