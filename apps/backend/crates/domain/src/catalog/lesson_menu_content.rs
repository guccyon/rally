use super::{LessonFormat, LessonMenuDescription, LessonMenuName};
use crate::catalog::Sport;

/// レッスンメニューの中身（コーチが作るとき・直すときに書くもの。公開中かは別に渡す）。
///
/// 各項目は検証済みの型で持つ。スポーツがコーチの提供スポーツかは [`super::LessonMenu`] が確かめる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonMenuContent {
    pub sport: Sport,
    pub format: LessonFormat,
    pub name: LessonMenuName,
    pub description: LessonMenuDescription,
}
