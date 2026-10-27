use crate::Unavailable;

/// [`super::LessonReviewRepository::insert`] の失敗。
#[derive(Debug)]
pub enum InsertReviewError {
    /// 同じレッスンの評価がすでにある（並行な評価で、後から保存しようとした側）
    LessonAlreadyReviewed,
    /// 保存先の障害
    Unavailable(Unavailable),
}
