//! review のユースケースとポート。レッスンの評価（glossary の lesson_review）と、コーチごとの評価の集計と一覧。
//!
//! 評価の対象の完了したレッスンは live-lesson / async-lesson が発行した事実で、review は
//! [`CompletedLessonLookup`] で読むだけ（実装は [`crate::lesson::LessonCompletionLookup`]）。評価の一覧に添える
//! レッスンメニューも、完了したレッスンの元（予約・動画）が持つ写しを [`ReviewedLessonMenuLookup`] で読むだけ
//! （実装は [`crate::lesson::LessonCompletionMenuLookup`]）。matching がコーチを探す一覧に添える評価の件数と平均は
//! [`ReviewCoachReviewSummaryLookup`] で、コーチの月ごとのレッスンの実績に添える暦月ごとの評価の件数と平均は
//! [`ReviewMonthlyReviewSummaryLookup`] で渡す。生徒が受けたレッスンの一覧に添える評価済みの印は
//! [`ReviewLessonCompletionReviewLookup`] で渡す。

mod coach_review_list;
mod completed_lesson_lookup;
mod get_coach_review_summary;
mod insert_review_error;
pub mod lesson_review_id_uuid;
mod list_coach_reviews;
mod repository;
mod review_coach_review_summary_lookup;
mod review_lesson_completion_review_lookup;
mod review_monthly_review_summary_lookup;
mod reviewed_lesson_menu_lookup;
mod submit_lesson_review;

pub use coach_review_list::CoachReviewList;
pub use completed_lesson_lookup::CompletedLessonLookup;
pub use get_coach_review_summary::GetCoachReviewSummary;
pub use insert_review_error::InsertReviewError;
pub use list_coach_reviews::ListCoachReviews;
pub use repository::LessonReviewRepository;
pub use review_coach_review_summary_lookup::ReviewCoachReviewSummaryLookup;
pub use review_lesson_completion_review_lookup::ReviewLessonCompletionReviewLookup;
pub use review_monthly_review_summary_lookup::ReviewMonthlyReviewSummaryLookup;
pub use reviewed_lesson_menu_lookup::ReviewedLessonMenuLookup;
pub use submit_lesson_review::{
    SubmitLessonReview, SubmitLessonReviewCommand, SubmitLessonReviewError,
};
