use domain::scheduling::SchedulingError;

use crate::Unavailable;

/// コーチの受付を止めた・再開した記録の照会（[`super::ListCoachAcceptanceChanges`]）の失敗。
#[derive(Debug)]
pub enum ListCoachAcceptanceChangesError {
    /// 期間が規則を満たさない（本部が送ったお知らせの一覧と同じ [`domain::scheduling::BookingPeriod::new`] の違反）
    Invalid(SchedulingError),
    /// 保存先・profile への問い合わせの障害
    Unavailable(Unavailable),
}

impl From<SchedulingError> for ListCoachAcceptanceChangesError {
    fn from(error: SchedulingError) -> Self {
        Self::Invalid(error)
    }
}

impl From<Unavailable> for ListCoachAcceptanceChangesError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}
