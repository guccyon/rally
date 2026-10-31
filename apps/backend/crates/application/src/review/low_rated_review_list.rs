use std::collections::BTreeMap;

use domain::{
    identity::AccountId,
    lesson::LessonCompletionId,
    profile::DisplayName,
    review::{LessonReview, ReviewLessonMenu},
};

/// 本部が見る低い評価の一覧（[`super::ListLowRatedReviews`] の結果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LowRatedReviewList {
    /// 低い評価（選び方と並べ方は domain の [`domain::review::LowRatedReviewCriteria`]）
    pub reviews: Vec<LessonReview>,
    /// `reviews` の評価した完了したレッスンのレッスンメニュー（レッスンの ID ごと）。メニューのないレッスンは含まない
    pub lesson_menus: BTreeMap<LessonCompletionId, ReviewLessonMenu>,
    /// `reviews` の評価されたコーチの表示名（コーチの ID ごと）。引けないコーチは含まない
    pub coach_display_names: BTreeMap<AccountId, DisplayName>,
    /// `reviews` の評価した生徒の表示名（生徒の ID ごと）。引けない生徒は含まない
    pub student_display_names: BTreeMap<AccountId, DisplayName>,
}
