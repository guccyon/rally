use crate::Unavailable;

/// 本部が見る生徒 1 人の受け取ったお知らせの一覧（[`super::ListStudentAnnouncements`]）の失敗。
#[derive(Debug)]
pub enum ListStudentAnnouncementsError {
    /// 生徒のアカウントがない（ID のアカウントがないか、生徒でない）
    StudentNotFound,
    /// 保存先の障害（お知らせの送った記録がないなど、保存先の破損も含む）
    Unavailable(Unavailable),
}

impl From<Unavailable> for ListStudentAnnouncementsError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}
