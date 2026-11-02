//! catalog の値と、DB に保存する文字列の対応表。
//!
//! `catalog.lesson_menus.lesson_type`（CHECK 制約の値）はこの表だけを通す。行動イベント `lesson.completed` の
//! `lesson_type` 属性も同じ値（[`crate::analytics_event_payload::lesson_type_value`] はこれを指す）。CHECK との一致は
//! `tests/schema.rs` が確かめる。`catalog.lesson_menu_publication_changes.action`（本部が非公開にした・公開に戻した記録の操作）も
//! この表だけを通す。

use domain::{
    catalog::{HeadquartersUnpublishReason, LessonMenuPublicationAction},
    lesson::LessonType,
};

/// `catalog.lesson_menus.lesson_type` の値（student-app の契約の `LessonType` と同じ）。
pub const fn lesson_type_value(lesson_type: LessonType) -> &'static str {
    match lesson_type {
        LessonType::Live => "live_lesson",
        LessonType::Async => "async_lesson",
    }
}

pub fn lesson_type_from_value(value: &str) -> Option<LessonType> {
    LessonType::ALL
        .into_iter()
        .find(|lesson_type| lesson_type_value(*lesson_type) == value)
}

/// `catalog.lesson_menu_publication_changes.action` の値。非公開にした記録の理由は値に含めない（`reason` の列）。
pub const fn lesson_menu_publication_action_value(
    action: &LessonMenuPublicationAction,
) -> &'static str {
    match action {
        LessonMenuPublicationAction::Unpublished { .. } => "unpublished",
        LessonMenuPublicationAction::Republished => "republished",
    }
}

/// `catalog.lesson_menu_publication_changes` の `action` と `reason` の列から。非公開にした記録は理由を持ち、公開に戻した記録は
/// 持たない（`lesson_menu_publication_changes_reason_shape`）。値と理由の組が合わなければ `None`。
pub fn lesson_menu_publication_action_from_value(
    value: &str,
    reason: Option<HeadquartersUnpublishReason>,
) -> Option<LessonMenuPublicationAction> {
    match (value, reason) {
        ("unpublished", Some(reason)) => Some(LessonMenuPublicationAction::Unpublished { reason }),
        ("republished", None) => Some(LessonMenuPublicationAction::Republished),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_lesson_menu_publication_action() {
        let unpublished = LessonMenuPublicationAction::Unpublished {
            reason: HeadquartersUnpublishReason::new("誤った記載").unwrap(),
        };
        assert_eq!(
            lesson_menu_publication_action_from_value(
                lesson_menu_publication_action_value(&unpublished),
                Some(HeadquartersUnpublishReason::new("誤った記載").unwrap())
            ),
            Some(unpublished)
        );
        let republished = LessonMenuPublicationAction::Republished;
        assert_eq!(
            lesson_menu_publication_action_from_value(
                lesson_menu_publication_action_value(&republished),
                None
            ),
            Some(republished)
        );
        assert_eq!(
            lesson_menu_publication_action_from_value("unpublished", None),
            None
        );
        assert_eq!(
            lesson_menu_publication_action_from_value(
                "republished",
                Some(HeadquartersUnpublishReason::default())
            ),
            None
        );
        assert_eq!(
            lesson_menu_publication_action_from_value("withdrawn", None),
            None
        );
    }

    #[test]
    fn round_trips_every_lesson_type() {
        for lesson_type in LessonType::ALL {
            assert_eq!(
                lesson_type_from_value(lesson_type_value(lesson_type)),
                Some(lesson_type)
            );
        }
        assert_eq!(lesson_type_from_value("live"), None);
    }
}
