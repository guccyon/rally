use std::time::SystemTime;

use domain::{identity::AccountId, scheduling::BookingId};

/// レッスンのリマインドを作る対象（確定した同期レッスンの予約 1 件）。持ち主は scheduling で、notification は読むだけ
/// （[`super::LessonReminderTargetLookup`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LessonReminderTarget {
    pub booking_id: BookingId,
    /// 予約した生徒
    pub student_id: AccountId,
    /// 予約を受けたコーチ（担当コーチ）
    pub coach_id: AccountId,
    /// 予約した枠の開始時刻
    pub starts_at: SystemTime,
}
