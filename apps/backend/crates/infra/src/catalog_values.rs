//! catalog の値と、DB に保存する文字列の対応表。
//!
//! `catalog.lesson_menus.lesson_type`（CHECK 制約の値）はこの表だけを通す。CHECK との一致は `tests/schema.rs` が
//! 確かめる。`sport` の列は identity と同じ値（[`crate::identity_values::sport_value`]）。

use domain::lesson::LessonType;

/// `catalog.lesson_menus.lesson_type` の値（glossary の live_lesson / async_lesson）。
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
