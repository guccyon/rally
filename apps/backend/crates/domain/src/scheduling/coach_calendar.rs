use super::Slot;
use crate::{DomainError, identity::AccountId};

/// 1 人のコーチの枠の集まり（集約）。同じコーチの枠が重ならないことをここで判定する。
///
/// 保存先から読むときは、少なくとも公開しようとする期間に重なる枠を含めればよい
/// （それ以外の枠は判定に影響しない）。閉じた枠（[`super::SlotStatus::Closed`]）は時間を占めない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachCalendar {
    coach_id: AccountId,
    slots: Vec<Slot>,
}

impl CoachCalendar {
    /// コーチの既存の枠から作る。別のコーチの枠が混ざっていれば [`DomainError::SlotOfAnotherCoach`]。
    pub fn new(
        coach_id: AccountId,
        slots: impl IntoIterator<Item = Slot>,
    ) -> Result<Self, DomainError> {
        let slots: Vec<Slot> = slots.into_iter().collect();
        if slots.iter().any(|slot| slot.coach_id() != coach_id) {
            return Err(DomainError::SlotOfAnotherCoach);
        }
        Ok(Self { coach_id, slots })
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }

    /// 枠を加える。別のコーチの枠なら [`DomainError::SlotOfAnotherCoach`]、時間を占める既存の枠と
    /// 重なれば [`DomainError::SlotOverlaps`]（重なった枠の ID を載せる）。
    pub fn publish(&mut self, slot: Slot) -> Result<(), DomainError> {
        if slot.coach_id() != self.coach_id {
            return Err(DomainError::SlotOfAnotherCoach);
        }
        if let Some(existing) = self
            .slots
            .iter()
            .filter(|existing| existing.status().occupies_time() && slot.status().occupies_time())
            .find(|existing| existing.overlaps(&slot))
        {
            return Err(DomainError::SlotOverlaps {
                existing: existing.id(),
            });
        }
        self.slots.push(slot);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::{
        identity::Sport,
        scheduling::{SlotId, SlotStatus},
    };

    const COACH: AccountId = AccountId::from_u128(1);

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(hour * 3600)
    }

    fn slot(id: u128, coach: AccountId, from: u64, to: u64, status: SlotStatus) -> Slot {
        Slot::restore(
            SlotId::from_u128(id),
            coach,
            Sport::Tennis,
            at(from),
            at(to),
            status,
            crate::scheduling::SlotSeats::default(),
        )
        .unwrap()
    }

    #[test]
    fn publishes_slots_that_do_not_overlap() {
        let mut calendar =
            CoachCalendar::new(COACH, [slot(1, COACH, 10, 11, SlotStatus::Open)]).unwrap();
        calendar
            .publish(slot(2, COACH, 11, 12, SlotStatus::Open))
            .unwrap();
        calendar
            .publish(slot(3, COACH, 8, 10, SlotStatus::Open))
            .unwrap();
        assert_eq!(calendar.slots().len(), 3);
    }

    #[test]
    fn overlapping_slot_is_rejected() {
        let mut calendar = CoachCalendar::new(
            COACH,
            [
                slot(1, COACH, 10, 11, SlotStatus::Open),
                slot(2, COACH, 13, 14, SlotStatus::Booked),
            ],
        )
        .unwrap();
        for (id, from, to, existing) in [(3, 10, 11, 1), (4, 9, 12, 1), (5, 13, 15, 2)] {
            assert_eq!(
                calendar.publish(slot(id, COACH, from, to, SlotStatus::Open)),
                Err(DomainError::SlotOverlaps {
                    existing: SlotId::from_u128(existing)
                })
            );
        }
        assert_eq!(calendar.slots().len(), 2);
    }

    #[test]
    fn closed_slot_does_not_occupy_time() {
        let mut calendar =
            CoachCalendar::new(COACH, [slot(1, COACH, 10, 11, SlotStatus::Closed)]).unwrap();
        calendar
            .publish(slot(2, COACH, 10, 11, SlotStatus::Open))
            .unwrap();
    }

    #[test]
    fn slots_of_another_coach_are_rejected() {
        let other = AccountId::from_u128(2);
        assert_eq!(
            CoachCalendar::new(COACH, [slot(1, other, 10, 11, SlotStatus::Open)]),
            Err(DomainError::SlotOfAnotherCoach)
        );
        let mut calendar = CoachCalendar::new(COACH, []).unwrap();
        assert_eq!(
            calendar.publish(slot(1, other, 10, 11, SlotStatus::Open)),
            Err(DomainError::SlotOfAnotherCoach)
        );
    }
}
