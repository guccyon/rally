use super::MonthlyLessonStats;

/// コーチの今月と先月のレッスンの実績（[`super::GetMyLessonStats`] の結果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachLessonStats {
    /// 今の時刻を含む暦月（日本時間）
    pub current_month: MonthlyLessonStats,
    /// その前の暦月
    pub previous_month: MonthlyLessonStats,
}
