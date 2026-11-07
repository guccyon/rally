use std::collections::BTreeSet;

use super::{BookingId, BookingPeriod, CoachLeaveNote};
use crate::identity::AccountId;

/// 本部がコーチをお休みにする依頼（[`super::CoachLeave::start`] の入力）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachLeaveRequest {
    /// お休みにするコーチ
    pub coach_id: AccountId,
    /// お休みの期間（日本時間の始まりの日の 0 時から、終わりの日の翌日の 0 時まで）
    pub period: BookingPeriod,
    /// 生徒への一言
    pub note: CoachLeaveNote,
    /// 操作した本部のアカウント
    pub requested_by: AccountId,
    /// 本部が画面で確かめた、期間の有効な予約の ID（[`super::CoachLeaveTargets::booking_ids`]）。今の予約と違えば断る
    pub expected_booking_ids: BTreeSet<BookingId>,
}
