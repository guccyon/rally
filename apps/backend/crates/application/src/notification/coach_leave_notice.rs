use std::time::SystemTime;

use domain::{profile::DisplayName, scheduling::CoachLeaveNote};

/// 本部が予約をキャンセルした（コーチのお休み）ことの生徒へのお知らせ
/// （[`domain::notification::PushNotification::BookingCancelledByHeadquarters`]）に添える中身。表示する本文は境界がここから
/// 決める。持ち主は scheduling（お休みの記録）と profile（コーチの表示名）で、notification は読むだけ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachLeaveNotice {
    /// お休みにしたコーチの表示名（引けなければ `None`）
    pub coach_display_name: Option<DisplayName>,
    /// キャンセルした予約の枠の開始時刻
    pub starts_at: SystemTime,
    /// キャンセルした予約の枠の終了時刻
    pub ends_at: SystemTime,
    /// 本部が添えた一言（書かなければ空）
    pub note: CoachLeaveNote,
}
