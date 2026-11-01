use std::time::SystemTime;

use super::BookingStatus;

/// 予約（[`super::Booking`]）の状態が変わった記録 1 件。変えた後の状態と、変わった時刻を持つ。
///
/// 作るのは [`super::Booking::status_changes`] の 1 か所で、予約が持つ確定・キャンセルの時刻から組み立てる
/// （状態の移り変わりを別に保存してはいない）。作った時刻（[`BookingStatus::Pending`] になった時刻）は予約の
/// `created_at` で、ここには含めない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BookingStatusChange {
    status: BookingStatus,
    changed_at: SystemTime,
}

impl BookingStatusChange {
    pub(super) const fn new(status: BookingStatus, changed_at: SystemTime) -> Self {
        Self { status, changed_at }
    }

    /// 変えた後の状態（[`BookingStatus::Confirmed`] か [`BookingStatus::Cancelled`]）。
    pub const fn status(&self) -> BookingStatus {
        self.status
    }

    pub const fn changed_at(&self) -> SystemTime {
        self.changed_at
    }
}
