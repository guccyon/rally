use crate::Unavailable;

/// [`super::LiveLessonSessionRepository::insert`] の失敗。
#[derive(Debug)]
pub enum InsertSessionError {
    /// 同じ予約のセッションがすでにある（並行な開始で、後から保存しようとした側）
    BookingAlreadyHasSession,
    /// 保存先の障害
    Unavailable(Unavailable),
}
