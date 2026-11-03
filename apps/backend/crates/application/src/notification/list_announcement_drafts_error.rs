use domain::scheduling::SchedulingError;

use crate::Unavailable;

/// 本部からのお知らせの下書きの一覧（[`super::ListAnnouncementDrafts`]）の失敗。
#[derive(Debug)]
pub enum ListAnnouncementDraftsError {
    /// 期間が規則を満たさない（送ったお知らせの一覧と同じ [`domain::scheduling::BookingPeriod::new`] の違反）
    Invalid(SchedulingError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<SchedulingError> for ListAnnouncementDraftsError {
    fn from(error: SchedulingError) -> Self {
        Self::Invalid(error)
    }
}

impl From<Unavailable> for ListAnnouncementDraftsError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}
