use std::time::SystemTime;

use super::{BookingCancelReason, BookingCanceller, BookingStatus, CancellationDecision};

/// 予約（[`super::Booking`]）の状態が変わった記録 1 件。変えた後の状態と、変わった時刻を持つ。キャンセルの記録は、
/// キャンセルの経緯（キャンセルした人・理由・キャンセル規定の判定）も持つ（どれも記録がなければ `None`）。
///
/// 作るのは [`super::Booking::status_changes`] の 1 か所で、予約が持つ確定・キャンセルの時刻と経緯から組み立てる
/// （状態の移り変わりを別に保存してはいない）。作った時刻（[`BookingStatus::Pending`] になった時刻）は予約の
/// `created_at` で、ここには含めない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BookingStatusChange {
    status: BookingStatus,
    changed_at: SystemTime,
    cancelled_by: Option<BookingCanceller>,
    cancel_reason: Option<BookingCancelReason>,
    cancellation_decision: Option<CancellationDecision>,
}

impl BookingStatusChange {
    pub(super) const fn new(status: BookingStatus, changed_at: SystemTime) -> Self {
        Self {
            status,
            changed_at,
            cancelled_by: None,
            cancel_reason: None,
            cancellation_decision: None,
        }
    }

    /// `changed_at` にキャンセルした記録。
    pub(super) const fn cancelled(
        changed_at: SystemTime,
        cancelled_by: Option<BookingCanceller>,
        cancel_reason: Option<BookingCancelReason>,
        cancellation_decision: Option<CancellationDecision>,
    ) -> Self {
        Self {
            status: BookingStatus::Cancelled,
            changed_at,
            cancelled_by,
            cancel_reason,
            cancellation_decision,
        }
    }

    /// 変えた後の状態（[`BookingStatus::Confirmed`] か [`BookingStatus::Cancelled`]）。
    pub const fn status(&self) -> BookingStatus {
        self.status
    }

    pub const fn changed_at(&self) -> SystemTime {
        self.changed_at
    }

    /// キャンセルした人（キャンセルの記録だけ。[`super::Booking::cancelled_by`]）。
    pub const fn cancelled_by(&self) -> Option<BookingCanceller> {
        self.cancelled_by
    }

    /// キャンセルした理由（キャンセルの記録だけ。[`super::Booking::cancel_reason`]）。
    pub const fn cancel_reason(&self) -> Option<BookingCancelReason> {
        self.cancel_reason
    }

    /// キャンセルしたときのキャンセル規定の判定（キャンセルの記録だけ。[`super::Booking::cancellation_decision`]）。
    pub const fn cancellation_decision(&self) -> Option<CancellationDecision> {
        self.cancellation_decision
    }
}
