use std::collections::BTreeMap;

use domain::{catalog::LessonMenu, identity::AccountId, profile::DisplayName};

/// コーチのレッスンメニューの横断の一覧（[`super::ListLessonMenus`] の結果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachLessonMenuList {
    /// 絞り込んだメニュー。スポーツの順 → コーチの表示名の順（引けないコーチは最後）→ コーチの ID → 作った時刻の古い順
    /// → メニューの ID の順
    pub lesson_menus: Vec<LessonMenu>,
    /// `lesson_menus` を作ったコーチの表示名（コーチの ID ごと）。表示名が引けないコーチは含まない
    pub coaches: BTreeMap<AccountId, DisplayName>,
}
