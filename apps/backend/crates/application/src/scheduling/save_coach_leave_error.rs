use domain::scheduling::SchedulingError;

use crate::Unavailable;

/// [`super::CoachLeaveRepository::start_coach_leave`] の失敗。
#[derive(Debug)]
pub enum SaveCoachLeaveError {
    /// お休みが不変条件を満たさない（クロージャが返した違反）
    Rejected(SchedulingError),
    /// 保存先の障害
    Unavailable(Unavailable),
}
