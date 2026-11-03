use domain::scheduling::SchedulingError;

use crate::Unavailable;

/// コーチが受けた評価の一覧（[`super::ListCoachReviews`]）の失敗。
#[derive(Debug)]
pub enum ListCoachReviewsError {
    /// 期間が規則を満たさない（本部が送ったお知らせの一覧と同じ [`domain::scheduling::BookingPeriod::new`] の違反）
    Invalid(SchedulingError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<SchedulingError> for ListCoachReviewsError {
    fn from(error: SchedulingError) -> Self {
        Self::Invalid(error)
    }
}

impl From<Unavailable> for ListCoachReviewsError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}
