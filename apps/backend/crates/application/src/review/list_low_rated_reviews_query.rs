use domain::{
    catalog::{LessonMenuId, Sport},
    identity::AccountId,
};
use time::Date;

/// 本部が見る低い評価の一覧（[`super::ListLowRatedReviews`]）の絞り込み。どれも省ける（すべて省けば絞らない）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ListLowRatedReviewsQuery {
    /// このスポーツのレッスンの評価だけ
    pub sport: Option<Sport>,
    /// 本部の対応済みの印があるか（`true`）・ないか（`false`）がこれと同じ評価だけ
    pub handled: Option<bool>,
    /// このコーチが評価された評価だけ
    pub coach_id: Option<AccountId>,
    /// この生徒が評価した評価だけ
    pub student_id: Option<AccountId>,
    /// 送った期間の始まりの日付（日本時間。含む）
    pub from: Option<Date>,
    /// 送った期間の終わりの日付（日本時間。含む）
    pub to: Option<Date>,
    /// 評価に添えるレッスンメニュー（完了したレッスンの元が持つ写し）がこのメニューの評価だけ
    pub lesson_menu_id: Option<LessonMenuId>,
}
