//! レッスンの完了記録（glossary の lesson_completion）。同期レッスン（live-lesson）と非同期レッスン（async-lesson）が共有する値。
//!
//! 境界づけられたコンテキストではない（specs/context-map.yaml に足さない）。完了記録（[`LessonCompletion`]）は
//! live-lesson の [`crate::live_lesson::LiveLessonSession::complete`] と async-lesson の
//! [`crate::async_lesson::Video::complete`] がそれぞれの集約から発行する（glossary の `contexts` は
//! live-lesson と async-lesson の両方。docs/adr/0006）。

mod lesson_completion;
mod lesson_completion_id;
mod lesson_source;
mod lesson_type;

pub use lesson_completion::LessonCompletion;
pub use lesson_completion_id::LessonCompletionId;
pub use lesson_source::LessonSource;
pub use lesson_type::LessonType;
