use super::LessonType;
use crate::{async_lesson::VideoId, live_lesson::SessionId};

/// 完了したレッスンの元になった集約。同期レッスンは通話セッション、非同期レッスンは添削を返した動画。
///
/// 種類と元の ID を 1 つの値で持つので、食い違った組み合わせ（同期レッスンなのに動画など）は作れない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LessonSource {
    /// 同期レッスンの通話セッション
    Live(SessionId),
    /// 非同期レッスンの動画
    Async(VideoId),
}

impl LessonSource {
    pub const fn lesson_type(self) -> LessonType {
        match self {
            LessonSource::Live(_) => LessonType::Live,
            LessonSource::Async(_) => LessonType::Async,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_source_decides_the_lesson_type() {
        assert_eq!(
            LessonSource::Live(SessionId::from_u128(1)).lesson_type(),
            LessonType::Live
        );
        assert_eq!(
            LessonSource::Async(VideoId::from_u128(1)).lesson_type(),
            LessonType::Async
        );
    }
}
