//! catalog の値と、DB に保存する文字列の対応表。
//!
//! `catalog.lesson_menus.lesson_type`（CHECK 制約の値）はこの表だけを通す。行動イベント `lesson.completed` の
//! `lesson_type` 属性も同じ値（[`crate::analytics_event_payload::lesson_type_value`] はこれを指す）。CHECK との一致は
//! `tests/schema.rs` が確かめる。

use domain::lesson::LessonType;

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

#[cfg(test)]
mod tests {
    use super::*;

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
