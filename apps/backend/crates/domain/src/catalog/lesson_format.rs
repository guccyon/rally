use super::LessonDuration;
use crate::{DomainError, lesson::LessonType};

/// レッスンメニューの形式。同期レッスンは 1 回の時間を持ち、非同期レッスン（動画添削）は持たない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LessonFormat {
    /// 同期レッスン（ビデオ通話）。1 回の時間
    Live(LessonDuration),
    /// 非同期レッスン（動画添削）
    Async,
}

impl LessonFormat {
    /// レッスンの種類と 1 回の時間から作る。同期なら時間が要り、非同期なら時間を持てない
    /// （違えば [`DomainError::LessonDurationMismatchesType`]）。
    pub fn new(
        lesson_type: LessonType,
        duration: Option<LessonDuration>,
    ) -> Result<Self, DomainError> {
        match (lesson_type, duration) {
            (LessonType::Live, Some(duration)) => Ok(Self::Live(duration)),
            (LessonType::Async, None) => Ok(Self::Async),
            _ => Err(DomainError::LessonDurationMismatchesType { lesson_type }),
        }
    }

    pub const fn lesson_type(self) -> LessonType {
        match self {
            Self::Live(_) => LessonType::Live,
            Self::Async => LessonType::Async,
        }
    }

    /// 同期レッスンの 1 回の時間。非同期なら `None`。
    pub const fn duration(self) -> Option<LessonDuration> {
        match self {
            Self::Live(duration) => Some(duration),
            Self::Async => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minutes(m: i64) -> LessonDuration {
        LessonDuration::from_minutes(m).unwrap()
    }

    #[test]
    fn live_lessons_have_a_duration_and_async_lessons_do_not() {
        let live = LessonFormat::new(LessonType::Live, Some(minutes(45))).unwrap();
        assert_eq!(live, LessonFormat::Live(minutes(45)));
        assert_eq!(live.lesson_type(), LessonType::Live);
        assert_eq!(live.duration(), Some(minutes(45)));

        let async_lesson = LessonFormat::new(LessonType::Async, None).unwrap();
        assert_eq!(async_lesson, LessonFormat::Async);
        assert_eq!(async_lesson.lesson_type(), LessonType::Async);
        assert_eq!(async_lesson.duration(), None);
    }

    #[test]
    fn duration_must_match_the_lesson_type() {
        assert_eq!(
            LessonFormat::new(LessonType::Live, None),
            Err(DomainError::LessonDurationMismatchesType {
                lesson_type: LessonType::Live
            })
        );
        assert_eq!(
            LessonFormat::new(LessonType::Async, Some(minutes(30))),
            Err(DomainError::LessonDurationMismatchesType {
                lesson_type: LessonType::Async
            })
        );
    }
}
