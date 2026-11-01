use std::time::SystemTime;

use super::{CoachAcceptanceChange, CoachAcceptanceStatus, CoachSuspensionReason, SchedulingError};
use crate::identity::AccountId;

/// コーチの受付（glossary の coach_acceptance）。コーチが新しい予約を受け付けるか（受付中・停止中）を持つ集約。
///
/// 止める・再開するのは本部（誰が呼べるかは境界が確かめる）。受付を止めても入っている予約（未確定・確定）と枠は
/// そのままで、変わるのは新しい予約を受けるか（[`Self::ensure_accepting_bookings`]）と、枠が生徒の検索とコーチを
/// 探す一覧に出るかだけ。一度も止めていないコーチは受付中（[`Self::accepting`]）。止めた・再開したときは、誰がいつ
/// 変えたかの記録（[`CoachAcceptanceChange`]）を返す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachAcceptance {
    coach_id: AccountId,
    status: CoachAcceptanceStatus,
}

impl CoachAcceptance {
    /// 受付中のコーチの受付（一度も止めていないコーチの既定）。
    pub const fn accepting(coach_id: AccountId) -> Self {
        Self {
            coach_id,
            status: CoachAcceptanceStatus::Accepting,
        }
    }

    /// 保存した状態から戻す。
    pub const fn restore(coach_id: AccountId, status: CoachAcceptanceStatus) -> Self {
        Self { coach_id, status }
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    pub const fn status(&self) -> CoachAcceptanceStatus {
        self.status
    }

    /// 受付を止める（受付中 → 停止中）。止めた本部のアカウント `changed_by` と理由 `reason`（書かなければ空）と時刻
    /// `changed_at` の記録を返す。すでに停止中なら [`SchedulingError::CoachAlreadySuspended`] で、何も変えず記録も作らない。
    pub fn suspend(
        &mut self,
        changed_by: AccountId,
        reason: CoachSuspensionReason,
        changed_at: SystemTime,
    ) -> Result<CoachAcceptanceChange, SchedulingError> {
        if self.status == CoachAcceptanceStatus::Suspended {
            return Err(SchedulingError::CoachAlreadySuspended);
        }
        Ok(self.change_to(
            CoachAcceptanceStatus::Suspended,
            Some(reason),
            changed_by,
            changed_at,
        ))
    }

    /// 受付を再開する（停止中 → 受付中）。再開した本部のアカウント `changed_by` と時刻 `changed_at` の記録を返す。
    /// 停止中でなければ [`SchedulingError::CoachNotSuspended`] で、何も変えず記録も作らない。
    pub fn resume(
        &mut self,
        changed_by: AccountId,
        changed_at: SystemTime,
    ) -> Result<CoachAcceptanceChange, SchedulingError> {
        if self.status != CoachAcceptanceStatus::Suspended {
            return Err(SchedulingError::CoachNotSuspended);
        }
        Ok(self.change_to(
            CoachAcceptanceStatus::Accepting,
            None,
            changed_by,
            changed_at,
        ))
    }

    fn change_to(
        &mut self,
        status: CoachAcceptanceStatus,
        reason: Option<CoachSuspensionReason>,
        changed_by: AccountId,
        changed_at: SystemTime,
    ) -> CoachAcceptanceChange {
        self.status = status;
        CoachAcceptanceChange::restore(self.coach_id, status, reason, changed_by, changed_at)
    }

    /// 新しい予約を受けられるか。停止中なら [`SchedulingError::CoachNotAcceptingBookings`]。
    pub fn ensure_accepting_bookings(&self) -> Result<(), SchedulingError> {
        match self.status {
            CoachAcceptanceStatus::Accepting => Ok(()),
            CoachAcceptanceStatus::Suspended => Err(SchedulingError::CoachNotAcceptingBookings),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COACH: AccountId = AccountId::from_u128(7);
    const HEADQUARTERS: AccountId = AccountId::from_u128(1);

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs)
    }

    fn reason(text: &str) -> CoachSuspensionReason {
        CoachSuspensionReason::new(text).unwrap()
    }

    #[test]
    fn a_coach_accepts_bookings_until_suspended() {
        let acceptance = CoachAcceptance::accepting(COACH);
        assert_eq!(acceptance.coach_id(), COACH);
        assert_eq!(acceptance.status(), CoachAcceptanceStatus::Accepting);
        assert_eq!(acceptance.ensure_accepting_bookings(), Ok(()));
    }

    #[test]
    fn suspending_stops_new_bookings_and_records_the_reason() {
        let mut acceptance = CoachAcceptance::accepting(COACH);
        assert_eq!(
            acceptance.suspend(HEADQUARTERS, reason(" 休業の申し出 "), at(10)),
            Ok(CoachAcceptanceChange::restore(
                COACH,
                CoachAcceptanceStatus::Suspended,
                Some(reason(" 休業の申し出 ")),
                HEADQUARTERS,
                at(10)
            ))
        );
        assert_eq!(acceptance.status(), CoachAcceptanceStatus::Suspended);
        assert_eq!(
            acceptance.ensure_accepting_bookings(),
            Err(SchedulingError::CoachNotAcceptingBookings)
        );
    }

    #[test]
    fn suspending_a_suspended_coach_is_rejected_without_change() {
        let mut acceptance = CoachAcceptance::restore(COACH, CoachAcceptanceStatus::Suspended);
        assert_eq!(
            acceptance.suspend(HEADQUARTERS, reason("二度目"), at(10)),
            Err(SchedulingError::CoachAlreadySuspended)
        );
        assert_eq!(acceptance.status(), CoachAcceptanceStatus::Suspended);
    }

    #[test]
    fn resuming_accepts_new_bookings_again() {
        let mut acceptance = CoachAcceptance::restore(COACH, CoachAcceptanceStatus::Suspended);
        let change = acceptance.resume(HEADQUARTERS, at(20)).unwrap();
        assert_eq!(change.coach_id(), COACH);
        assert_eq!(change.status(), CoachAcceptanceStatus::Accepting);
        assert_eq!(change.reason(), None, "再開した記録は理由を持たない");
        assert_eq!(change.changed_by(), HEADQUARTERS);
        assert_eq!(change.changed_at(), at(20));
        assert_eq!(acceptance.status(), CoachAcceptanceStatus::Accepting);
        assert_eq!(acceptance.ensure_accepting_bookings(), Ok(()));
    }

    #[test]
    fn resuming_an_accepting_coach_is_rejected_without_change() {
        let mut acceptance = CoachAcceptance::accepting(COACH);
        assert_eq!(
            acceptance.resume(HEADQUARTERS, at(20)),
            Err(SchedulingError::CoachNotSuspended)
        );
        assert_eq!(acceptance.status(), CoachAcceptanceStatus::Accepting);
    }
}
