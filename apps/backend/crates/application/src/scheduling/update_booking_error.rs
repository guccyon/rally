use domain::DomainError;

use crate::Unavailable;

/// [`super::BookingRepository::update`] の失敗。
#[derive(Debug)]
pub enum UpdateBookingError {
    /// 予約がない
    BookingNotFound,
    /// 変更が不変条件を満たさない（クロージャが返した違反）
    Rejected(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}
