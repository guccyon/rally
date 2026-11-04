use domain::{catalog::Sport, identity::AccountId, profile::DisplayNameQuery};
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
    /// あればこのコーチの枠を一度でも予約した生徒だけ（予約の状態を問わない。数え方は domain の
    /// [`domain::scheduling::CoachBookedStudents`]）。ないコーチ・コーチでない ID なら誰も残らない
    pub coach_id: Option<AccountId>,
    /// `true` なら返信のないまま既定の時間の過ぎたスレッドが 1 件以上ある生徒だけ（数え方は生徒 1 人の利用の様子と同じ
    /// domain の [`domain::messaging::StudentUnansweredMessageThreadCount`]、判定は
    /// [`domain::messaging::UnansweredStudentFilter`]）。`false` なら絞らない
    pub has_unanswered: bool,
}
