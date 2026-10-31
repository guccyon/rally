use domain::DomainError;

use crate::Unavailable;

/// 本部によるレッスンメニューの非公開（[`super::UnpublishLessonMenu`]）と公開に戻す操作（[`super::RepublishLessonMenu`]）の失敗。
#[derive(Debug)]
pub enum ChangeLessonMenuPublicationError {
    /// メニューがない
    LessonMenuNotFound,
    /// 変更がドメインの不変条件を満たさない（本部が非公開にしたメニューの非公開、本部が非公開にしていないメニューを
    /// 公開に戻す）
    Invalid(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<Unavailable> for ChangeLessonMenuPublicationError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}
