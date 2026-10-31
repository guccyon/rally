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
}
