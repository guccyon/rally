//! review のユースケースとポート。レッスンの評価（glossary の lesson_review）と、評価へのコーチの返信
//! （glossary の lesson_review_reply。[`ReplyToLessonReview`]）と、コーチごとの評価の集計と一覧。
//!
//! 評価の対象の完了したレッスンは live-lesson / async-lesson が発行した事実で、review は
//! [`CompletedLessonLookup`] で読むだけ（実装は [`crate::lesson::LessonCompletionLookup`]）。評価の一覧に添える
//! レッスンメニューも、完了したレッスンの元（予約・動画）が持つ写しを [`ReviewedLessonMenuLookup`] で読むだけ
//! （実装は [`crate::lesson::LessonCompletionMenuLookup`]）。matching がコーチを探す一覧に添える評価の件数と平均は
//! [`ReviewCoachReviewSummaryLookup`] で、コーチの月ごとのレッスンの実績に添える暦月ごとの評価の件数と平均は
//! [`ReviewMonthlyReviewSummaryLookup`] で渡す。生徒が受けたレッスンの一覧に添える評価済みの印は
//! [`ReviewLessonCompletionReviewLookup`] で渡す。本部が見る低い評価の一覧（[`ListLowRatedReviews`]）に添えるコーチ・生徒の
//! 表示名は profile から [`ReviewedCoachDisplayNameLookup`]・[`ReviewerDisplayNameLookup`] で読むだけ。

mod coach_review_list;
mod completed_lesson_lookup;
mod get_coach_review_summary;
mod insert_review_error;
pub mod lesson_review_id_uuid;
mod list_coach_reviews;
mod list_low_rated_reviews;
mod low_rated_review_list;
mod reply_to_lesson_review;
mod repository;
mod review_coach_review_summary_lookup;
mod review_lesson_completion_review_lookup;
mod review_monthly_review_summary_lookup;
mod reviewed_coach_display_name_lookup;
mod reviewed_lesson_menu_lookup;
mod reviewer_display_name_lookup;
mod submit_lesson_review;

pub use coach_review_list::CoachReviewList;
pub use completed_lesson_lookup::CompletedLessonLookup;
pub use get_coach_review_summary::GetCoachReviewSummary;
pub use insert_review_error::InsertReviewError;
pub use list_coach_reviews::ListCoachReviews;
pub use list_low_rated_reviews::ListLowRatedReviews;
pub use low_rated_review_list::LowRatedReviewList;
pub use reply_to_lesson_review::{
    ReplyToLessonReview, ReplyToLessonReviewCommand, ReplyToLessonReviewError,
};
pub use repository::LessonReviewRepository;
pub use review_coach_review_summary_lookup::ReviewCoachReviewSummaryLookup;
pub use review_lesson_completion_review_lookup::ReviewLessonCompletionReviewLookup;
pub use review_monthly_review_summary_lookup::ReviewMonthlyReviewSummaryLookup;
pub use reviewed_coach_display_name_lookup::ReviewedCoachDisplayNameLookup;
pub use reviewed_lesson_menu_lookup::ReviewedLessonMenuLookup;
pub use reviewer_display_name_lookup::ReviewerDisplayNameLookup;
pub use submit_lesson_review::{
    SubmitLessonReview, SubmitLessonReviewCommand, SubmitLessonReviewError,
};
