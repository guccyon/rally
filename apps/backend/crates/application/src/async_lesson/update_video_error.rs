use domain::DomainError;

use crate::Unavailable;

/// [`super::VideoRepository::update`] の失敗。
#[derive(Debug)]
pub enum UpdateVideoError {
    /// 動画がない
    NotFound,
    /// 変更が不変条件を満たさない（クロージャが返した違反）
    Rejected(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}
