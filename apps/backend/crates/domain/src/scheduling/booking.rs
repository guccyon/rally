use std::time::SystemTime;

use super::{
    BookingId, BookingStatus, CancellationDecision, CancellationPolicy, Slot, SlotId, SlotStatus,
};
use crate::{DomainError, identity::AccountId};

/// 予約（glossary の booking）。生徒によるスロットの確保で、体験予約（`is_trial`）もこれ。
///
/// 作成した直後は [`BookingStatus::Pending`] で、確定（[`Booking::confirm`]）するとスロットが
/// [`SlotStatus::Booked`] になる。キャンセル（[`Booking::cancel`]）すると [`BookingStatus::Cancelled`] になり、
/// スロットは公開中に戻る。コーチと開始時刻は予約したスロットのもの。
/// 予約しても受講権は消費しない（product-context の用語の注意）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Booking {
    id: BookingId,
    student_id: AccountId,
    slot_id: SlotId,
    coach_id: AccountId,
    is_trial: bool,
    status: BookingStatus,
    starts_at: SystemTime,
    created_at: SystemTime,
    confirmed_at: Option<SystemTime>,
    cancelled_at: Option<SystemTime>,
}

impl Booking {
    /// 生徒 `student_id` が `slot` を予約する（状態は [`BookingStatus::Pending`]）。
    ///
    /// `existing` には少なくとも、`slot` の予約と `student_id` の生徒の予約を含める（それ以外の予約は
    /// 判定に影響しない）。次の不変条件を破れば [`DomainError`] を返す。
    ///
    /// - 公開中（[`SlotStatus::Open`]）のスロットにしか予約できない（[`DomainError::SlotNotOpen`]）
    /// - 1 つのスロットに有効な予約は 1 件まで（定員 [`Slot::CAPACITY`]。[`DomainError::SlotAlreadyBooked`]）
    /// - 体験予約はその生徒の初回の予約だけ。キャンセルされていない予約が 1 件でもあれば
    ///   [`DomainError::TrialAlreadyUsed`]
    pub fn reserve(
        id: BookingId,
        student_id: AccountId,
        is_trial: bool,
        slot: &Slot,
        existing: &[Booking],
        now: SystemTime,
    ) -> Result<Self, DomainError> {
        if slot.status() != SlotStatus::Open {
            return Err(DomainError::SlotNotOpen {
                status: slot.status(),
            });
        }
        let active = || existing.iter().filter(|b| b.status.is_active());
        if active().filter(|b| b.slot_id == slot.id()).count() >= slot.capacity() as usize {
            return Err(DomainError::SlotAlreadyBooked);
        }
        if is_trial && active().any(|b| b.student_id == student_id) {
            return Err(DomainError::TrialAlreadyUsed);
        }
        Ok(Self {
            id,
            student_id,
            slot_id: slot.id(),
            coach_id: slot.coach_id(),
            is_trial,
            status: BookingStatus::Pending,
            starts_at: slot.starts_at(),
            created_at: now,
            confirmed_at: None,
            cancelled_at: None,
        })
    }

    /// 予約した生徒 `student_id` が確定する。予約したスロット `slot` を [`SlotStatus::Booked`] にする。
    ///
    /// 別の生徒なら [`DomainError::BookingOfAnotherStudent`]、確定前（[`BookingStatus::Pending`]）で
    /// なければ [`DomainError::BookingNotPending`]、スロットが公開中でなければ
    /// [`DomainError::SlotNotOpen`]。失敗したときは予約もスロットも変えない。
    pub fn confirm(
        &mut self,
        student_id: AccountId,
        slot: &mut Slot,
        now: SystemTime,
    ) -> Result<(), DomainError> {
        if student_id != self.student_id {
            return Err(DomainError::BookingOfAnotherStudent);
        }
        if slot.id() != self.slot_id {
            return Err(DomainError::BookingOfAnotherSlot);
        }
        if self.status != BookingStatus::Pending {
            return Err(DomainError::BookingNotPending {
                status: self.status,
            });
        }
        slot.book()?;
        self.status = BookingStatus::Confirmed;
        self.confirmed_at = Some(now);
        Ok(())
    }

    /// 予約した生徒 `student_id` が `now` にキャンセルし、キャンセル規定の判定を返す。
    ///
    /// 判定は [`CancellationPolicy::decide`] だけが行う（回数の消費の実行は entitlement）。確定済みなら
    /// 予約したスロット `slot` を公開中（[`SlotStatus::Open`]）に戻す。確定前ならスロットはもともと公開中。
    /// 別の生徒なら [`DomainError::BookingOfAnotherStudent`]、キャンセル済みなら
    /// [`DomainError::BookingAlreadyCancelled`]、確定済みなのにスロットが予約済みでなければ
    /// [`DomainError::SlotNotBooked`]。失敗したときは予約もスロットも変えない。
    pub fn cancel(
        &mut self,
        student_id: AccountId,
        slot: &mut Slot,
        now: SystemTime,
    ) -> Result<CancellationDecision, DomainError> {
        if student_id != self.student_id {
            return Err(DomainError::BookingOfAnotherStudent);
        }
        if slot.id() != self.slot_id {
            return Err(DomainError::BookingOfAnotherSlot);
        }
        let decision = match self.status {
            BookingStatus::Cancelled => return Err(DomainError::BookingAlreadyCancelled),
            BookingStatus::Pending => CancellationPolicy::decide(self, now),
            BookingStatus::Confirmed => {
                slot.release()?;
                CancellationPolicy::decide(self, now)
            }
        };
        self.status = BookingStatus::Cancelled;
        self.cancelled_at = Some(now);
        Ok(decision)
    }

    pub const fn id(&self) -> BookingId {
        self.id
    }

    pub const fn student_id(&self) -> AccountId {
        self.student_id
    }

    pub const fn slot_id(&self) -> SlotId {
        self.slot_id
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    pub const fn is_trial(&self) -> bool {
        self.is_trial
    }

    pub const fn status(&self) -> BookingStatus {
        self.status
    }

    /// 予約したスロットの開始時刻。
    pub const fn starts_at(&self) -> SystemTime {
        self.starts_at
    }

    pub const fn created_at(&self) -> SystemTime {
        self.created_at
    }

    /// 確定した時刻。確定するまでは `None`。
    pub const fn confirmed_at(&self) -> Option<SystemTime> {
        self.confirmed_at
    }

    /// キャンセルした時刻。キャンセルするまでは `None`。
    pub const fn cancelled_at(&self) -> Option<SystemTime> {
        self.cancelled_at
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::identity::Sport;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(3);
    const NOW: SystemTime = SystemTime::UNIX_EPOCH;

    fn at(hour: u64) -> SystemTime {
        NOW + Duration::from_secs(hour * 3600)
    }

    fn slot(id: u128, status: SlotStatus) -> Slot {
        Slot::restore(
            SlotId::from_u128(id),
            COACH,
            Sport::Tennis,
            at(id as u64),
            at(id as u64 + 1),
            status,
        )
        .unwrap()
    }

    fn reserve(
        id: u128,
        student: AccountId,
        is_trial: bool,
        slot: &Slot,
        existing: &[Booking],
    ) -> Result<Booking, DomainError> {
        Booking::reserve(
            BookingId::from_u128(id),
            student,
            is_trial,
            slot,
            existing,
            NOW,
        )
    }

    fn cancelled(mut booking: Booking) -> Booking {
        booking.status = BookingStatus::Cancelled;
        booking.cancelled_at = Some(NOW);
        booking
    }

    #[test]
    fn cancelling_a_confirmed_booking_reopens_the_slot_and_returns_the_decision() {
        let mut slot = slot(10, SlotStatus::Open);
        let mut booking = reserve(1, STUDENT, false, &slot, &[]).unwrap();
        booking.confirm(STUDENT, &mut slot, NOW).unwrap();

        // 開始（10 時）の 1 時間前。判定は CancellationPolicy と同じ
        let decision = booking.cancel(STUDENT, &mut slot, at(9)).unwrap();

        assert_eq!(decision, CancellationPolicy::decide(&booking, at(9)));
        assert!(decision.consumes_quota);
        assert_eq!(booking.status(), BookingStatus::Cancelled);
        assert_eq!(booking.cancelled_at(), Some(at(9)));
        assert_eq!(slot.status(), SlotStatus::Open, "スロットは公開中に戻る");
    }

    #[test]
    fn cancelling_a_pending_booking_keeps_the_slot_open() {
        let mut slot = slot(100, SlotStatus::Open);
        let mut booking = reserve(1, STUDENT, true, &slot, &[]).unwrap();

        // 開始（100 時）の 99 時間前
        let decision = booking.cancel(STUDENT, &mut slot, at(1)).unwrap();

        assert!(!decision.consumes_quota);
        assert_eq!(booking.status(), BookingStatus::Cancelled);
        assert_eq!(booking.cancelled_at(), Some(at(1)));
        assert_eq!(slot.status(), SlotStatus::Open);
    }

    #[test]
    fn cancel_failures_change_nothing() {
        let open = slot(10, SlotStatus::Open);
        let pending = reserve(1, STUDENT, false, &open, &[]).unwrap();
        let mut booked = open.clone();
        let mut confirmed = pending.clone();
        confirmed.confirm(STUDENT, &mut booked, NOW).unwrap();

        for (mut booking, student, mut slot, expected) in [
            (
                pending.clone(),
                OTHER_STUDENT,
                open.clone(),
                DomainError::BookingOfAnotherStudent,
            ),
            (
                pending.clone(),
                STUDENT,
                self::slot(11, SlotStatus::Open),
                DomainError::BookingOfAnotherSlot,
            ),
            (
                cancelled(pending.clone()),
                STUDENT,
                open.clone(),
                DomainError::BookingAlreadyCancelled,
            ),
            (
                confirmed.clone(),
                STUDENT,
                open.clone(),
                DomainError::SlotNotBooked {
                    status: SlotStatus::Open,
                },
            ),
        ] {
            let (before, slot_before) = (booking.clone(), slot.clone());
            assert_eq!(booking.cancel(student, &mut slot, at(2)), Err(expected));
            assert_eq!((booking, slot), (before, slot_before));
        }
    }

    #[test]
    fn reserves_an_open_slot_as_pending() {
        let slot = slot(10, SlotStatus::Open);
        let booking = reserve(1, STUDENT, true, &slot, &[]).unwrap();
        assert_eq!(booking.status(), BookingStatus::Pending);
        assert_eq!(booking.slot_id(), slot.id());
        assert_eq!(booking.coach_id(), COACH);
        assert_eq!(booking.starts_at(), at(10));
        assert_eq!(booking.created_at(), NOW);
        assert_eq!(booking.confirmed_at(), None);
        assert!(booking.is_trial());
    }

    #[test]
    fn only_open_slots_can_be_reserved() {
        for status in [SlotStatus::Booked, SlotStatus::Closed] {
            assert_eq!(
                reserve(1, STUDENT, false, &slot(10, status), &[]),
                Err(DomainError::SlotNotOpen { status })
            );
        }
    }

    #[test]
    fn slot_with_an_active_booking_cannot_be_reserved_again() {
        let slot = slot(10, SlotStatus::Open);
        let first = reserve(1, STUDENT, false, &slot, &[]).unwrap();
        for student in [STUDENT, OTHER_STUDENT] {
            assert_eq!(
                reserve(2, student, false, &slot, std::slice::from_ref(&first)),
                Err(DomainError::SlotAlreadyBooked)
            );
        }
        // キャンセルされた予約はスロットを確保しない
        reserve(2, OTHER_STUDENT, false, &slot, &[cancelled(first)]).unwrap();
        // 別のスロットの予約は関係ない
        let other = reserve(
            3,
            OTHER_STUDENT,
            false,
            &self::slot(11, SlotStatus::Open),
            &[],
        );
        reserve(4, STUDENT, false, &slot, &[other.unwrap()]).unwrap();
    }

    #[test]
    fn trial_is_only_for_the_first_booking_of_the_student() {
        let first_slot = slot(10, SlotStatus::Open);
        let next_slot = slot(11, SlotStatus::Open);
        let trial = reserve(1, STUDENT, true, &first_slot, &[]).unwrap();
        assert_eq!(
            reserve(2, STUDENT, true, &next_slot, std::slice::from_ref(&trial)),
            Err(DomainError::TrialAlreadyUsed)
        );
        // 通常の予約の後でも体験予約はできない
        let regular = reserve(3, OTHER_STUDENT, false, &first_slot, &[]).unwrap();
        assert_eq!(
            reserve(4, OTHER_STUDENT, true, &next_slot, &[regular]),
            Err(DomainError::TrialAlreadyUsed)
        );
        // 2 件目でも通常の予約はできる
        reserve(5, STUDENT, false, &next_slot, std::slice::from_ref(&trial)).unwrap();
        // 別の生徒の予約は関係ない
        reserve(
            6,
            OTHER_STUDENT,
            true,
            &next_slot,
            std::slice::from_ref(&trial),
        )
        .unwrap();
        // キャンセルした予約は初回に数えない
        reserve(7, STUDENT, true, &next_slot, &[cancelled(trial)]).unwrap();
    }

    #[test]
    fn confirming_books_the_slot() {
        let mut slot = slot(10, SlotStatus::Open);
        let mut booking = reserve(1, STUDENT, true, &slot, &[]).unwrap();

        booking.confirm(STUDENT, &mut slot, at(1)).unwrap();

        assert_eq!(booking.status(), BookingStatus::Confirmed);
        assert_eq!(booking.confirmed_at(), Some(at(1)));
        assert_eq!(slot.status(), SlotStatus::Booked);
    }

    #[test]
    fn confirm_failures_change_nothing() {
        let open = slot(10, SlotStatus::Open);
        let pending = reserve(1, STUDENT, false, &open, &[]).unwrap();

        let mut confirmed = pending.clone();
        confirmed
            .confirm(STUDENT, &mut open.clone(), at(1))
            .unwrap();
        let mut closed = slot(10, SlotStatus::Closed);

        for (mut booking, student, mut slot, expected) in [
            (
                pending.clone(),
                OTHER_STUDENT,
                open.clone(),
                DomainError::BookingOfAnotherStudent,
            ),
            (
                pending.clone(),
                STUDENT,
                self::slot(11, SlotStatus::Open),
                DomainError::BookingOfAnotherSlot,
            ),
            (
                confirmed.clone(),
                STUDENT,
                open.clone(),
                DomainError::BookingNotPending {
                    status: BookingStatus::Confirmed,
                },
            ),
            (
                cancelled(pending.clone()),
                STUDENT,
                open.clone(),
                DomainError::BookingNotPending {
                    status: BookingStatus::Cancelled,
                },
            ),
            (
                pending.clone(),
                STUDENT,
                closed.clone(),
                DomainError::SlotNotOpen {
                    status: SlotStatus::Closed,
                },
            ),
        ] {
            let (before, slot_before) = (booking.clone(), slot.clone());
            assert_eq!(booking.confirm(student, &mut slot, at(2)), Err(expected));
            assert_eq!((booking, slot), (before, slot_before));
        }
        assert_eq!(
            closed.book(),
            Err(DomainError::SlotNotOpen {
                status: SlotStatus::Closed
            })
        );
    }
}
