use domain::catalog::LessonMenu;

/// メニューの一覧の並べ方（admin-web の `listMyLessonMenus`・student-app の `listCoachLessonMenus` の説明）:
/// 作った時刻の古い順、同じ時刻は ID の順。
pub(super) fn sort_for_listing(menus: &mut [LessonMenu]) {
    menus.sort_by_key(|menu| (menu.created_at(), menu.id()));
}
