use domain::scheduling::SchedulingError;

use crate::Unavailable;

/// 本部が送ったお知らせの一覧（[`super::ListAnnouncements`]）の失敗。
#[derive(Debug)]
pub enum ListAnnouncementsError {
    /// 期間が規則を満たさない（本部が見る予約の一覧と同じ [`domain::scheduling::BookingPeriod::new`] の違反）
    Invalid(SchedulingError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<SchedulingError> for ListAnnouncementsError {
    fn from(error: SchedulingError) -> Self {
        Self::Invalid(error)
    }
}

impl From<Unavailable> for ListAnnouncementsError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}
