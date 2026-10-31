use crate::catalog::{LessonMenu, LessonMenuId, LessonMenuName};

/// 動画を送った時点の非同期レッスンのレッスンメニュー（catalog の [`LessonMenu`] の写し）。動画が持つのは ID・名前で、
/// 送った後にメニューが直されても変わらない（動画の表示は送った時点のまま）。
///
/// 写してよいかの判定は [`super::Video::with_lesson_menu`] の 1 か所。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoLessonMenu {
    id: LessonMenuId,
    name: LessonMenuName,
}

impl VideoLessonMenu {
    /// `menu` の今の ID・名前を写す。
    pub fn copy_of(menu: &LessonMenu) -> Self {
        Self {
            id: menu.id(),
            name: menu.name().clone(),
        }
    }

    /// 保存した写しを読み戻す（保存先の実装用）。
    pub const fn restore(id: LessonMenuId, name: LessonMenuName) -> Self {
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
    use std::time::SystemTime;

    use super::*;
    use crate::{
        catalog::Sport,
        catalog::{LessonFormat, LessonMenuContent, LessonMenuDescription},
        identity::AccountId,
    };

    fn menu(name: &str) -> LessonMenu {
        LessonMenu::restore(
            LessonMenuId::from_u128(5),
            AccountId::from_u128(7),
            LessonMenuContent {
                sport: Sport::Tennis,
                format: LessonFormat::Async,
                name: LessonMenuName::new(name).unwrap(),
                description: LessonMenuDescription::new("説明").unwrap(),
            },
            true,
            SystemTime::UNIX_EPOCH,
        )
    }

    #[test]
    fn copies_the_id_and_name_of_the_menu() {
        let copy = VideoLessonMenu::copy_of(&menu("フォーム添削"));
        assert_eq!(copy.id(), LessonMenuId::from_u128(5));
        assert_eq!(copy.name().as_str(), "フォーム添削");
        assert_eq!(
            copy,
            VideoLessonMenu::restore(
                LessonMenuId::from_u128(5),
                LessonMenuName::new("フォーム添削").unwrap()
            )
        );
    }

    #[test]
    fn the_copy_does_not_follow_later_revisions_of_the_menu() {
        let copy = VideoLessonMenu::copy_of(&menu("フォーム添削"));
        let revised = menu("スイング添削");
        assert_eq!(copy.name().as_str(), "フォーム添削");
        assert_ne!(copy, VideoLessonMenu::copy_of(&revised));
    }
}
