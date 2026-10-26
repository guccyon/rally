//! レッスンの完了記録のユースケースとポート（live-lesson と async-lesson が共有する。docs/adr/0006）。
//!
//! 境界づけられたコンテキストではない。完了記録は通話セッション（live-lesson）・動画（async-lesson）の集約が
//! 発行し、ここはそれを保存して行動イベントと境界間イベントを出すだけ。

pub mod lesson_id_uuid;
mod record_completion_error;
mod record_lesson_completion;
mod repository;

pub use record_completion_error::RecordCompletionError;
pub use record_lesson_completion::{
    RecordLessonCompletion, RecordLessonCompletionCommand, RecordLessonCompletionError,
};
pub use repository::LessonCompletionRepository;
