use domain::scheduling::SchedulingError;

use super::UpdateCoachAcceptanceError;
use crate::Unavailable;

/// コーチの受付の停止（[`super::SuspendCoach`]）と再開（[`super::ResumeCoach`]）の失敗。
#[derive(Debug)]
pub enum ChangeCoachAcceptanceError {
    /// コーチがいない（コーチでないアカウント・ないアカウント）
    CoachNotFound,
    /// 変更がドメインの不変条件を満たさない（停止中のコーチの停止、受付中のコーチの再開）
    Invalid(SchedulingError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<UpdateCoachAcceptanceError> for ChangeCoachAcceptanceError {
    fn from(error: UpdateCoachAcceptanceError) -> Self {
        match error {
            UpdateCoachAcceptanceError::Rejected(error) => Self::Invalid(error),
            UpdateCoachAcceptanceError::Unavailable(error) => Self::Unavailable(error),
        }
    }
}

impl From<Unavailable> for ChangeCoachAcceptanceError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}
