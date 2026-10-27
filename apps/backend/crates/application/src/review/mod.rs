//! review のユースケースとポート。レッスンの評価（glossary の lesson_review）と、コーチごとの評価の集計。
//!
//! 評価の対象の完了したレッスンは live-lesson / async-lesson が発行した事実で、review は
//! [`CompletedLessonLookup`] で読むだけ（実装は [`crate::lesson::LessonCompletionLookup`]）。

mod completed_lesson_lookup;
mod get_coach_review_summary;
mod insert_review_error;
pub mod lesson_review_id_uuid;
mod repository;
mod submit_lesson_review;

pub use completed_lesson_lookup::CompletedLessonLookup;
pub use get_coach_review_summary::GetCoachReviewSummary;
pub use insert_review_error::InsertReviewError;
pub use repository::LessonReviewRepository;
pub use submit_lesson_review::{
    SubmitLessonReview, SubmitLessonReviewCommand, SubmitLessonReviewError,
};
