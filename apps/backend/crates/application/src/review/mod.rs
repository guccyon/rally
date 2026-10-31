//! review のユースケースとポート。レッスンの評価（glossary の lesson_review）と、コーチごとの評価の集計と一覧。
//!
//! 評価の対象の完了したレッスンは live-lesson / async-lesson が発行した事実で、review は
//! [`CompletedLessonLookup`] で読むだけ（実装は [`crate::lesson::LessonCompletionLookup`]）。評価の一覧に添える
//! レッスンメニューも、完了したレッスンの元（予約・動画）が持つ写しを [`ReviewedLessonMenuLookup`] で読むだけ
//! （実装は [`crate::lesson::LessonCompletionMenuLookup`]）。

mod coach_review_list;
mod completed_lesson_lookup;
mod get_coach_review_summary;
mod insert_review_error;
pub mod lesson_review_id_uuid;
mod list_coach_reviews;
mod repository;
mod reviewed_lesson_menu_lookup;
mod submit_lesson_review;

pub use coach_review_list::CoachReviewList;
pub use completed_lesson_lookup::CompletedLessonLookup;
pub use get_coach_review_summary::GetCoachReviewSummary;
pub use insert_review_error::InsertReviewError;
pub use list_coach_reviews::ListCoachReviews;
pub use repository::LessonReviewRepository;
pub use reviewed_lesson_menu_lookup::ReviewedLessonMenuLookup;
pub use submit_lesson_review::{
    SubmitLessonReview, SubmitLessonReviewCommand, SubmitLessonReviewError,
};
