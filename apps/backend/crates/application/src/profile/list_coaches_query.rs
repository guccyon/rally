use domain::{catalog::Sport, profile::DisplayNameQuery, scheduling::CoachAcceptanceStatus};

/// 本部が見るコーチの一覧（[`super::ListCoaches`]）の条件。どれも省け、省いた条件では絞り込まない（すべて省けば
/// すべてのコーチ）。付けた条件はすべてを満たすコーチだけを残す。並べ方は条件で変わらない。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListCoachesQuery {
    /// あれば表示名がこれに部分一致するコーチだけ（一致の規則は [`DisplayNameQuery`]）
    pub name: Option<DisplayNameQuery>,
    /// あればこの受付の状態のコーチだけ
    pub acceptance: Option<CoachAcceptanceStatus>,
    /// あればこのスポーツを提供する（提供スポーツのどれかが一致する）コーチだけ
    pub sport: Option<Sport>,
    /// `true` なら直近の期間に完了したレッスンのない（`recent_lesson_count` が 0 の）コーチだけ（判定は
    /// [`domain::lesson::InactiveCoachFilter`]）。`false`（既定）なら絞らない
    pub inactive: bool,
}
