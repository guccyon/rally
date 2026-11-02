use time::Date;

/// 本部が送ったお知らせの一覧（[`super::ListAnnouncements`]）の条件。どれも省ける（両方省けば今までどおりすべて）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ListAnnouncementsQuery {
    /// 送った期間の始まりの日付（日本時間。含む）
    pub from: Option<Date>,
    /// 送った期間の終わりの日付（日本時間。含む）
    pub to: Option<Date>,
}
