use domain::notification::NotificationError;

use crate::Unavailable;

/// 本部からのお知らせの下書きのユースケース（[`super::CreateAnnouncementDraft`]・[`super::UpdateAnnouncementDraft`]・
/// [`super::DeleteAnnouncementDraft`]・[`super::SendAnnouncementDraft`]）の失敗。
#[derive(Debug)]
pub enum AnnouncementDraftError {
    /// 下書きがない
    DraftNotFound,
    /// 下書きか、下書きから送るお知らせが不変条件を満たさない（題・本文が長すぎる。送るときは空・受け取る生徒がいない）
    Invalid(NotificationError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<Unavailable> for AnnouncementDraftError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}
