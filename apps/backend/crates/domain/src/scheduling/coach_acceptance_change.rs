use std::time::SystemTime;

use super::{CoachAcceptanceStatus, CoachSuspensionReason};
use crate::identity::AccountId;

/// コーチの受付（[`super::CoachAcceptance`]）を止めた・再開した記録 1 件。どのコーチの受付を、誰（本部のアカウント）が、
/// いつ、どちらの状態にしたかを持つ。止めた記録は、止めたときに書いた理由（[`CoachSuspensionReason`]。書かなければ空）も持つ。
///
/// 作るのは [`super::CoachAcceptance::suspend`]・[`super::CoachAcceptance::resume`] が状態を変えたときだけ（違反で何も
/// 変えなかったときは作らない）。`status` は変えた後の状態で、停止中なら止めた記録、受付中なら再開した記録。
/// 保存した記録を戻すときは [`Self::restore`]。記録は後から書き換えない。理由は止めた記録だけが持ち、再開した記録と、理由を
/// 残し始める前に止めた記録は持たない（`None`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachAcceptanceChange {
    coach_id: AccountId,
    status: CoachAcceptanceStatus,
    reason: Option<CoachSuspensionReason>,
    changed_by: AccountId,
    changed_at: SystemTime,
}

impl CoachAcceptanceChange {
    /// 保存した記録から戻す。
    pub const fn restore(
        coach_id: AccountId,
        status: CoachAcceptanceStatus,
        reason: Option<CoachSuspensionReason>,
        changed_by: AccountId,
        changed_at: SystemTime,
    ) -> Self {
        Self {
            coach_id,
            status,
            reason,
            changed_by,
            changed_at,
        }
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    /// 変えた後の受付の状態（停止中なら止めた、受付中なら再開した）。
    pub const fn status(&self) -> CoachAcceptanceStatus {
        self.status
    }

    /// 止めたときに書いた理由（書かなければ空）。再開した記録と、理由を残し始める前に止めた記録は `None`。
    pub const fn reason(&self) -> Option<&CoachSuspensionReason> {
        self.reason.as_ref()
    }

    /// 止めた・再開した本部のアカウントの ID。
    pub const fn changed_by(&self) -> AccountId {
        self.changed_by
    }

    pub const fn changed_at(&self) -> SystemTime {
        self.changed_at
    }
}
