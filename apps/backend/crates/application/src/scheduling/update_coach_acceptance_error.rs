use domain::scheduling::SchedulingError;

use crate::Unavailable;

/// [`super::CoachAcceptanceRepository::update`] の失敗。
#[derive(Debug)]
pub enum UpdateCoachAcceptanceError {
    /// 変更が不変条件を満たさない（クロージャが返した違反）
    Rejected(SchedulingError),
    /// 保存先の障害
    Unavailable(Unavailable),
}
