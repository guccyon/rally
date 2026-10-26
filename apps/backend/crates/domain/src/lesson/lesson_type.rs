/// レッスンの種類（glossary の live_lesson / async_lesson）。外部での文字列表現は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LessonType {
    /// 同期レッスン（ビデオ通話）
    Live,
    /// 非同期レッスン（動画添削）
    Async,
}

impl LessonType {
    /// すべての種類。
    pub const ALL: [LessonType; 2] = [LessonType::Live, LessonType::Async];
}
