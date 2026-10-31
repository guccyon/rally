use domain::scheduling::SchedulingError;

use crate::Unavailable;

/// [`super::BookingRepository::reserve`] の失敗。
#[derive(Debug)]
pub enum ReserveSlotError {
    /// スロットがない
    SlotNotFound,
    /// 予約が不変条件を満たさない（クロージャが返した違反）
    Rejected(SchedulingError),
    /// 保存先の障害
    Unavailable(Unavailable),
}
