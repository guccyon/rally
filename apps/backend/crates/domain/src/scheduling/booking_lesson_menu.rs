use crate::catalog::{LessonDuration, LessonMenu, LessonMenuId, LessonMenuName};

/// 予約した時点のレッスンメニュー（catalog の [`LessonMenu`] の写し）。予約が持つのは ID・名前・1 回の時間で、
/// 予約した後にメニューが直されても変わらない（予約の表示は予約した時点のまま）。
///
/// 写すかどうかの判定は [`super::Booking::with_lesson_menu`] の 1 か所。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookingLessonMenu {
    id: LessonMenuId,
    name: LessonMenuName,
    duration: Option<LessonDuration>,
}

impl BookingLessonMenu {
    /// `menu` の今の ID・名前・1 回の時間を写す。
    pub fn copy_of(menu: &LessonMenu) -> Self {
        Self {
            id: menu.id(),
            name: menu.name().clone(),
            duration: menu.format().duration(),
        }
    }

    /// 保存した写しを読み戻す（保存先の実装用）。
    pub const fn restore(
        id: LessonMenuId,
        name: LessonMenuName,
        duration: Option<LessonDuration>,
    ) -> Self {
        Self { id, name, duration }
    }

    pub const fn id(&self) -> LessonMenuId {
        self.id
    }

    pub const fn name(&self) -> &LessonMenuName {
        &self.name
    }

    /// 同期レッスン 1 回の時間。予約した時点でメニューが非同期だったなら `None`。
    pub const fn duration(&self) -> Option<LessonDuration> {
        self.duration
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;
    use crate::{
        catalog::{LessonFormat, LessonMenuContent, LessonMenuDescription},
        identity::{AccountId, Sport},
    };

    fn menu(format: LessonFormat) -> LessonMenu {
        LessonMenu::restore(
            LessonMenuId::from_u128(5),
            AccountId::from_u128(7),
            LessonMenuContent {
                sport: Sport::Tennis,
                format,
                name: LessonMenuName::new("フォア改善").unwrap(),
                description: LessonMenuDescription::new("説明").unwrap(),
            },
            true,
            SystemTime::UNIX_EPOCH,
        )
    }

    #[test]
    fn copies_the_id_name_and_duration_of_the_menu() {
        let sixty = LessonDuration::from_minutes(60).unwrap();
        let copy = BookingLessonMenu::copy_of(&menu(LessonFormat::Live(sixty)));
        assert_eq!(copy.id(), LessonMenuId::from_u128(5));
        assert_eq!(copy.name().as_str(), "フォア改善");
        assert_eq!(copy.duration(), Some(sixty));

        // 非同期に直されたメニューは時間を持たない
        assert_eq!(
            BookingLessonMenu::copy_of(&menu(LessonFormat::Async)).duration(),
            None
        );
    }
}
