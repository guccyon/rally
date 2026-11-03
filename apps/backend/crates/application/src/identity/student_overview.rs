use std::time::SystemTime;

use domain::{catalog::Sport, identity::AccountId, profile::DisplayName};

use super::StudentContract;

/// 本部が見る生徒 1 人の様子（[`super::ListStudents`] の結果の 1 行）。メールアドレスは持たない（本部の一覧には出さない）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudentOverview {
    pub student_id: AccountId,
    pub display_name: DisplayName,
    pub sport: Sport,
    /// 受講権の契約種別と契約プラン（受講権がなければ `None`）
    pub contract: Option<StudentContract>,
    /// 最後に予約した時刻（予約がなければ `None`）
    pub last_booked_at: Option<SystemTime>,
    /// 直近 30 日に作った、キャンセルされていない予約の数（数え方は [`domain::scheduling::RecentStudentBookings`]。
    /// 予約がなければ 0）
    pub recent_booking_count: u32,
    pub registered_at: SystemTime,
}
