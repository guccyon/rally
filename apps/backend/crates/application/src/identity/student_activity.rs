use std::collections::BTreeMap;

use domain::{
    identity::AccountId, lesson::LessonCompletion, profile::DisplayName, scheduling::Booking,
};

/// 本部が見る生徒 1 人の利用の様子（[`super::GetStudentActivity`] の結果）。個人を特定する情報は表示名と ID だけで、
/// メールアドレスは持たない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudentActivity {
    pub student_id: AccountId,
    pub display_name: DisplayName,
    /// これからの予約（並べ方は [`super::StudentBookingActivity::upcoming`]）
    pub upcoming_bookings: Vec<Booking>,
    /// 直近の受けたレッスン（完了した時刻の新しい順。最大 [`super::GetStudentActivity::RECENT_LESSON_LIMIT`] 件）
    pub recent_lessons: Vec<LessonCompletion>,
    /// キャンセルした予約の件数（これまでのすべて）
    pub cancelled_booking_count: u32,
    /// `upcoming_bookings` と `recent_lessons` のコーチの表示名（コーチの ID ごと）。表示名が引けないコーチは含まない
    pub coaches: BTreeMap<AccountId, DisplayName>,
}
