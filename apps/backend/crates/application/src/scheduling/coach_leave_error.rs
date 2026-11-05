use domain::scheduling::SchedulingError;

use super::SaveCoachLeaveError;
use crate::Unavailable;

/// 本部が見る期間の予約と枠の確かめ（[`super::PreviewCoachLeave`]）と、お休みにする操作（[`super::StartCoachLeave`]）の失敗。
#[derive(Debug)]
pub enum CoachLeaveError {
    /// コーチがいない（コーチでないアカウント・ないアカウント）
    CoachNotFound,
    /// 期間・一言・お休みがドメインの不変条件を満たさない（期間の誤り・始まりが今日以前・重なるお休み・確かめた後に予約が
    /// 変わった）
    Invalid(SchedulingError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<SaveCoachLeaveError> for CoachLeaveError {
    fn from(error: SaveCoachLeaveError) -> Self {
        match error {
            SaveCoachLeaveError::Rejected(error) => Self::Invalid(error),
            SaveCoachLeaveError::Unavailable(error) => Self::Unavailable(error),
        }
    }
}

impl From<SchedulingError> for CoachLeaveError {
    fn from(error: SchedulingError) -> Self {
        Self::Invalid(error)
    }
}

impl From<Unavailable> for CoachLeaveError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}
