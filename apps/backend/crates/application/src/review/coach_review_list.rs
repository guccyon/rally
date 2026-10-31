use std::collections::BTreeMap;

use domain::{
    lesson::LessonCompletionId,
    review::{LessonReview, ReviewLessonMenu},
};

/// コーチが受けた評価の一覧（[`super::ListCoachReviews`] の結果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachReviewList {
    /// コーチへの評価。送った時刻の新しい順で、同じ時刻は評価の ID の順
    pub reviews: Vec<LessonReview>,
    /// `reviews` の評価した完了したレッスンのレッスンメニュー（レッスンの ID ごと）。メニューのないレッスンは含まない
    pub lesson_menus: BTreeMap<LessonCompletionId, ReviewLessonMenu>,
}
