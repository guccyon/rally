use domain::messaging::MessagingError;

use crate::Unavailable;

/// [`super::MessageThreadRepository::update`] の失敗。
#[derive(Debug)]
pub enum UpdateMessageThreadError {
    /// スレッドがない
    NotFound,
    /// 変更が不変条件を満たさない（クロージャが返した違反）
    Rejected(MessagingError),
    /// 保存先の障害
    Unavailable(Unavailable),
}
