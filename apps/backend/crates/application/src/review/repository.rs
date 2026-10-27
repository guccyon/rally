use std::future::Future;

use domain::{
    identity::AccountId,
    lesson::LessonId,
    review::{LessonReview, LessonReviewId},
};

use super::InsertReviewError;
use crate::Unavailable;

/// レッスンの評価の保存先のポート。実装はテスト・ローカル起動用の
/// [`crate::in_memory::InMemoryLessonReviewRepository`] と、infra の Postgres（`review.lesson_reviews`）。
///
/// 不変条件の判定は domain の [`LessonReview`] が行う。保存先は「完了したレッスン 1 件につき評価は 1 つ」を
/// 保存の時点でも守る（並行な評価が両方保存されないように。DB では `lesson_reviews_lesson_id_key`）。
pub trait LessonReviewRepository: Send + Sync {
    /// 新しい評価の ID を払い出す。
    fn next_id(&self) -> LessonReviewId;

    /// `lesson_id` のレッスンの評価。なければ `None`。
    fn find_by_lesson(
        &self,
        lesson_id: LessonId,
    ) -> impl Future<Output = Result<Option<LessonReview>, Unavailable>> + Send;

    /// 新しい評価を保存する。同じレッスンの評価がすでにあれば [`InsertReviewError::LessonAlreadyReviewed`]。
    fn insert(
        &self,
        review: LessonReview,
    ) -> impl Future<Output = Result<(), InsertReviewError>> + Send;

    /// コーチ `coach_id` への評価をすべて読む（数えない・並べない。並べるのは [`super::ListCoachReviews`]、数えるのは domain の
    /// [`domain::review::CoachReviewSummary::tally`]）。
    fn find_by_coach(
        &self,
        coach_id: AccountId,
    ) -> impl Future<Output = Result<Vec<LessonReview>, Unavailable>> + Send;
}
