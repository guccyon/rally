use domain::notification::NotificationError;

use crate::Unavailable;

/// 本部によるお知らせの取り下げ（[`super::WithdrawAnnouncement`]）の失敗。
#[derive(Debug)]
pub enum WithdrawAnnouncementError {
    /// お知らせがない
    AnnouncementNotFound,
    /// 取り下げがドメインの不変条件を満たさない（すでに取り下げてある）
    Invalid(NotificationError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<Unavailable> for WithdrawAnnouncementError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}
