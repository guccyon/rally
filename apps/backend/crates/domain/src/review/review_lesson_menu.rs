use crate::catalog::{LessonMenuId, LessonMenuName};

/// 評価した完了したレッスンのレッスンメニュー（ID・名前）。コーチが受けた評価の一覧で「どのメニューのレッスンへの
/// 評価か」を示す。
///
/// 中身は完了したレッスンの元（同期レッスンは予約、非同期レッスンは動画）が持つメニューの写しで、review は持たずに
/// 読むだけ（写したのは予約した・動画を送った時点で、その後にメニューが直されても変わらない）。元がメニューを
/// 持たなければ評価にもメニューはない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewLessonMenu {
    id: LessonMenuId,
    name: LessonMenuName,
}

impl ReviewLessonMenu {
    pub const fn new(id: LessonMenuId, name: LessonMenuName) -> Self {
        Self { id, name }
    }

    pub const fn id(&self) -> LessonMenuId {
        self.id
    }

    pub const fn name(&self) -> &LessonMenuName {
        &self.name
    }

    /// 本部の低い評価の一覧の任意のレッスンメニューの絞り込み `lesson_menu_id` に、メニューが `lesson_menu` の評価が入るか。
    /// `lesson_menu` は評価に添えるメニュー（完了したレッスンの元が持つ写し。元にメニューがなければ `None`）。指定があれば
    /// 写しの ID がそれと同じときだけ、なければどの評価も入る。メニューのない評価・ないメニューの ID は入らない（一覧は空に
    /// なる。予約の [`crate::scheduling::Booking::matches_lesson_menu`] と同じ扱い）。
    pub fn matches_lesson_menu(
        lesson_menu: Option<&Self>,
        lesson_menu_id: Option<LessonMenuId>,
    ) -> bool {
        lesson_menu_id.is_none_or(|id| lesson_menu.is_some_and(|menu| menu.id == id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_id_and_name_of_the_menu() {
        let menu = ReviewLessonMenu::new(
            LessonMenuId::from_u128(5),
            LessonMenuName::new("フォア改善").unwrap(),
        );
        assert_eq!(menu.id(), LessonMenuId::from_u128(5));
        assert_eq!(menu.name().as_str(), "フォア改善");
    }

    #[test]
    fn matches_lesson_menu_by_the_id_of_the_copy() {
        let menu = ReviewLessonMenu::new(
            LessonMenuId::from_u128(5),
            LessonMenuName::new("フォア改善").unwrap(),
        );
        assert!(
            ReviewLessonMenu::matches_lesson_menu(Some(&menu), None),
            "指定がなければ入る"
        );
        assert!(
            ReviewLessonMenu::matches_lesson_menu(None, None),
            "指定がなければメニューのない評価も入る"
        );
        assert!(ReviewLessonMenu::matches_lesson_menu(
            Some(&menu),
            Some(LessonMenuId::from_u128(5))
        ));
        assert!(
            !ReviewLessonMenu::matches_lesson_menu(Some(&menu), Some(LessonMenuId::from_u128(6))),
            "別のメニューは入らない"
        );
        assert!(
            !ReviewLessonMenu::matches_lesson_menu(None, Some(LessonMenuId::from_u128(5))),
            "メニューのない評価は指定があれば入らない"
        );
    }
}
