//! review: レッスンの評価（glossary の lesson_review）と、コーチごとの評価の集計（specs/context-map.yaml の review）。
//!
//! 評価の対象は完了したレッスン（[`crate::lesson::LessonCompletion`]。live-lesson と async-lesson が発行する事実）で、
//! review はそれを読むだけ。評価を付けられるのはそのレッスンの生徒だけで、完了したレッスン 1 件につき 1 回。
//! 本部が横断で見る低い評価のしきい値と期間は [`LowRatedReviewCriteria`] の 1 か所。
//! 評価されたコーチは評価 1 件に 1 つだけ返信（[`LessonReviewReply`]）を付けられ、付け直すと上書きする。
//! 本部は低い評価（[`LowRatedReviewCriteria`] の点）に「対応済み」の印（[`LowRatedReviewHandling`]）を 1 つだけ付けられ、
//! 2 回目は何も変えない。コーチ 1 人の直近の低い評価と、そのうち未対応の件数は [`CoachLowRatedReviewCount`] で数える。
//! 評価の一覧に添えるレッスンメニュー（[`ReviewLessonMenu`]）も、完了したレッスンの元（予約・動画）が持つ写しを読むだけ。

mod coach_low_rated_review_count;
mod coach_review_summary;
mod lesson_review;
mod lesson_review_comment;
mod lesson_review_id;
mod lesson_review_reply;
mod lesson_review_reply_comment;
mod low_rated_review_criteria;
mod low_rated_review_handling;
mod rating;
mod review_error;
mod review_lesson_menu;

pub use coach_low_rated_review_count::CoachLowRatedReviewCount;
pub use coach_review_summary::CoachReviewSummary;
pub use lesson_review::LessonReview;
pub use lesson_review_comment::LessonReviewComment;
pub use lesson_review_id::LessonReviewId;
pub use lesson_review_reply::LessonReviewReply;
pub use lesson_review_reply_comment::LessonReviewReplyComment;
pub use low_rated_review_criteria::LowRatedReviewCriteria;
pub use low_rated_review_handling::LowRatedReviewHandling;
pub use rating::Rating;
pub use review_error::ReviewError;
pub use review_lesson_menu::ReviewLessonMenu;
