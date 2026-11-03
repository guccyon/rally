use domain::notification::AnnouncementTitleQuery;
use time::Date;

/// 本部が送ったお知らせの一覧（[`super::ListAnnouncements`]）の条件。どれも省ける（すべて省けば今までどおりすべて）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListAnnouncementsQuery {
    /// 送った期間の始まりの日付（日本時間。含む）
    pub from: Option<Date>,
    /// 送った期間の終わりの日付（日本時間。含む）
    pub to: Option<Date>,
    /// 題の検索語（下書きの一覧と同じ [`AnnouncementTitleQuery`]）。あれば題に部分一致するものだけ
    pub title: Option<AnnouncementTitleQuery>,
}
