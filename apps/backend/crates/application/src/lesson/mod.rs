//! レッスンの完了記録のユースケースとポート（live-lesson と async-lesson が共有する。docs/adr/0006）。
//!
//! 境界づけられたコンテキストではない。完了記録は通話セッション（live-lesson）・動画（async-lesson）の集約が
//! 発行し、ここはそれを保存して行動イベントと境界間イベントを出すだけ。review には完了記録と、その元（予約・動画）が
//! 持つレッスンメニューの写しを読ませる（[`LessonCompletionLookup`]・[`LessonCompletionMenuLookup`]）。

pub mod lesson_completion_id_uuid;
mod lesson_completion_lookup;
mod lesson_completion_menu_lookup;
mod record_completion_error;
mod record_lesson_completion;
mod repository;

pub use lesson_completion_lookup::LessonCompletionLookup;
pub use lesson_completion_menu_lookup::LessonCompletionMenuLookup;
pub use record_completion_error::RecordCompletionError;
pub use record_lesson_completion::{
    RecordLessonCompletion, RecordLessonCompletionCommand, RecordLessonCompletionError,
};
pub use repository::LessonCompletionRepository;
