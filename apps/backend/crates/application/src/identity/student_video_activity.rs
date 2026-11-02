/// 生徒 1 人の動画添削の様子（[`super::StudentVideoActivityLookup`] の結果。本部が見る生徒 1 人の利用の様子に添える）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StudentVideoActivity {
    /// 生徒が送った動画の数（状態を問わず、期間で絞らない。これまでのすべて）
    pub submitted_count: u32,
    /// 送った動画のうち、問い合わせた時刻に返却を待っている動画の数（判定は domain の
    /// [`domain::async_lesson::PendingVideoReviewFilter`] の 1 か所）
    pub pending_review_count: u32,
}
