//! review: レッスンの評価（glossary の lesson_review）と、コーチごとの評価の集計（specs/context-map.yaml の review）。
//!
//! 評価の対象は完了したレッスン（[`crate::lesson::LessonCompletion`]。live-lesson と async-lesson が発行する事実）で、
//! review はそれを読むだけ。評価を付けられるのはそのレッスンの生徒だけで、完了したレッスン 1 件につき 1 回。
//! 評価の一覧に添えるレッスンメニュー（[`ReviewLessonMenu`]）も、完了したレッスンの元（予約・動画）が持つ写しを読むだけ。

mod coach_review_summary;
mod lesson_review;
mod lesson_review_comment;
mod lesson_review_id;
mod rating;
mod review_lesson_menu;

pub use coach_review_summary::CoachReviewSummary;
pub use lesson_review::LessonReview;
pub use lesson_review_comment::LessonReviewComment;
pub use lesson_review_id::LessonReviewId;
pub use rating::Rating;
pub use review_lesson_menu::ReviewLessonMenu;
