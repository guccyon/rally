use domain::DomainError;

use crate::Unavailable;

/// [`super::LessonCompletionRepository::record`] の失敗。
#[derive(Debug)]
pub enum RecordCompletionError {
    /// 記録が不変条件を満たさない（クロージャが返した違反）
    Rejected(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}
