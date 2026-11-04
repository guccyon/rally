use domain::{catalog::Sport, notification::AnnouncementTitleQuery};
use time::Date;

/// 本部からのお知らせの下書きの一覧（[`super::ListAnnouncementDrafts`]）の条件。どれも省ける（すべて省けば今までどおり
/// すべて）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListAnnouncementDraftsQuery {
    /// 最後に直した期間の始まりの日付（日本時間。含む）
    pub from: Option<Date>,
    /// 最後に直した期間の終わりの日付（日本時間。含む）
    pub to: Option<Date>,
    /// 題の検索語（[`AnnouncementTitleQuery`]）。あれば題に部分一致するものだけ
    pub title: Option<AnnouncementTitleQuery>,
    /// 対象のスポーツ。あればそのスポーツの生徒に向けた下書きだけ（生徒全員に向けた下書きは入らない。
    /// [`domain::notification::AnnouncementDraft::is_in_sport`]）
    pub sport: Option<Sport>,
}
