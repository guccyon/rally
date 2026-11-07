use std::collections::BTreeSet;

use domain::{
    identity::AccountId,
    scheduling::{BookingId, CoachLeaveNote},
};
use time::Date;

/// お休みにする操作（[`super::StartCoachLeave`]）の入力。`requested_by` は認証した本部のアカウント（役割の確認は境界が
/// 行う）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartCoachLeaveCommand {
    pub coach_id: AccountId,
    /// お休みの始まりの日（日本時間）
    pub from: Date,
    /// お休みの終わりの日（日本時間。含む）
    pub to: Date,
    pub note: CoachLeaveNote,
    pub requested_by: AccountId,
    /// 本部が画面で確かめた、期間の有効な予約の ID
    pub expected_booking_ids: BTreeSet<BookingId>,
}
