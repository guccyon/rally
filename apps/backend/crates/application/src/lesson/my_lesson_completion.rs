use domain::lesson::{LessonCompletion, LessonCompletionMenu};

/// 生徒が受けたレッスンの一覧の 1 件（[`super::MyLessonCompletionList`] の項目）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MyLessonCompletion {
    /// 完了したレッスン（同期・非同期の別と元はここから読む）
    pub completion: LessonCompletion,
    /// 完了記録の元（予約・動画）が持つレッスンメニューの写し。元がメニューを持たなければ `None`
    pub lesson_menu: Option<LessonCompletionMenu>,
    /// 生徒がこの完了したレッスンに評価を付けたか
    pub reviewed: bool,
}
