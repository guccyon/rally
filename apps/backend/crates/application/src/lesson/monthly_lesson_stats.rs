use domain::{entitlement::YearMonth, lesson::CoachLessonCount, review::CoachReviewSummary};

/// コーチ 1 人の 1 つの暦月（日本時間）のレッスンの実績。金額は持たない（支払い額は billing の持ち物）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthlyLessonStats {
    /// 暦月（日本時間）
    pub month: YearMonth,
    /// その月に完了したレッスンの数（同期・非同期の別。完了した時刻の暦月で数える）
    pub lessons: CoachLessonCount,
    /// その月に受けた評価の件数と平均（評価を送った時刻の暦月で数える）
    pub reviews: CoachReviewSummary,
}
