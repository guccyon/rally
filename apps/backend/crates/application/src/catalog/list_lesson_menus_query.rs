use domain::{
    catalog::{LessonMenuNameQuery, Sport},
    identity::AccountId,
};

/// 本部が見るレッスンメニューの横断の一覧（[`super::ListLessonMenus`]）の条件。どれも省け、省いた条件では絞り込まない
/// （すべて省けばすべてのメニュー）。付けた条件はすべてを満たすメニューだけを残す。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListLessonMenusQuery {
    /// あれば名前がこれに部分一致するメニューだけ
    pub name: Option<LessonMenuNameQuery>,
    /// あればこのスポーツのメニューだけ
    pub sport: Option<Sport>,
    /// あれば公開中（`true`）か非公開（`false`）のメニューだけ
    pub is_published: Option<bool>,
    /// あればこのコーチのメニューだけ（いないコーチなら空の一覧）
    pub coach_id: Option<AccountId>,
}
