use domain::{catalog::Sport, profile::DisplayNameQuery};
use time::Date;

/// 本部が見る生徒の一覧（[`super::ListStudents`]）の条件。どれも省け、省いた条件では絞り込まない（すべて省けば
/// すべての生徒）。付けた条件はすべてを満たす生徒だけを残す。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListStudentsQuery {
    /// あれば表示名がこれに部分一致する生徒だけ
    pub name: Option<DisplayNameQuery>,
    /// あればこのスポーツの生徒だけ
    pub sport: Option<Sport>,
    /// あればこの日付（日本時間）の始まり以降に予約を作っていない生徒だけ（一度も予約していない生徒を含む）
    pub not_booked_since: Option<Date>,
}
