use super::{LessonDuration, LessonMenuDescription, LessonMenuName};

/// レッスンメニューのうち、コーチが作るときに書き、あとから直せる中身（名前・説明・1 回の時間・公開中か）。
///
/// 1 回の時間がレッスンの種類（同期 / 非同期）に合うかは集約（[`super::LessonMenu`]）が確かめる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonMenuContent {
    pub name: LessonMenuName,
    pub description: LessonMenuDescription,
    /// 同期レッスンの 1 回の時間。非同期レッスンは `None`
    pub duration: Option<LessonDuration>,
    /// 公開中なら true（生徒の一覧に出る）。false は非公開
    pub is_published: bool,
}
