use domain::DomainError;

use crate::Unavailable;

/// [`super::LiveLessonSessionRepository::update`] の失敗。
#[derive(Debug)]
pub enum UpdateSessionError {
    /// セッションがない
    NotFound,
    /// 変更が不変条件を満たさない（クロージャが返した違反）
    Rejected(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}
