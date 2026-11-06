use std::time::SystemTime;

use super::BookingId;
use crate::identity::AccountId;

/// コーチのお休み（[`super::CoachLeave`]）で本部がキャンセルした予約 1 件の写し。どの予約をキャンセルしたかと、生徒に
/// 知らせる日時（予約した枠の開始と終了）をお休みの記録に残す。キャンセルした後に枠が変わっても写しは変わらない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoachLeaveBooking {
    booking_id: BookingId,
    student_id: AccountId,
    starts_at: SystemTime,
    ends_at: SystemTime,
}

impl CoachLeaveBooking {
    /// 写しを組み立てる（[`super::CoachLeave::start`] と保存先の実装用）。
    pub const fn new(
        booking_id: BookingId,
        student_id: AccountId,
        starts_at: SystemTime,
        ends_at: SystemTime,
    ) -> Self {
        Self {
            booking_id,
            student_id,
            starts_at,
            ends_at,
        }
    }

    pub const fn booking_id(&self) -> BookingId {
        self.booking_id
    }

    /// 予約した生徒（お知らせを受け取る人）。
    pub const fn student_id(&self) -> AccountId {
        self.student_id
    }

    /// 予約した枠の開始時刻。
    pub const fn starts_at(&self) -> SystemTime {
        self.starts_at
    }

    /// 予約した枠の終了時刻。
    pub const fn ends_at(&self) -> SystemTime {
        self.ends_at
    }
}
