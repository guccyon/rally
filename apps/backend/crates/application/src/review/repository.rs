use std::{future::Future, time::SystemTime};

use domain::{
    identity::AccountId,
    lesson::LessonCompletionId,
    review::{LessonReview, LessonReviewId, LessonReviewReply, LowRatedReviewHandling},
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

    /// 評価 `review_id`（返信を含む）。なければ `None`。
    fn find_by_id(
        &self,
        review_id: LessonReviewId,
    ) -> impl Future<Output = Result<Option<LessonReview>, Unavailable>> + Send;

    /// `lesson_id` のレッスンの評価。なければ `None`。
    fn find_by_lesson(
        &self,
        lesson_id: LessonCompletionId,
    ) -> impl Future<Output = Result<Option<LessonReview>, Unavailable>> + Send;

    /// 新しい評価を保存する。同じレッスンの評価がすでにあれば [`InsertReviewError::LessonAlreadyReviewed`]。
    fn insert(
        &self,
        review: LessonReview,
    ) -> impl Future<Output = Result<(), InsertReviewError>> + Send;

    /// 評価 `review_id` の返信を `reply` に置き換える（返信がなければ付ける）。点と一言は変えない。返信できるかの判定は
    /// domain の [`LessonReview::write_reply`] が済ませてある。評価がなければ何もしない。
    fn save_reply(
        &self,
        review_id: LessonReviewId,
        reply: &LessonReviewReply,
    ) -> impl Future<Output = Result<(), Unavailable>> + Send;

    /// 評価 `review_id` に本部の対応済みの印 `handling` を付ける。印がすでにあれば何もしない（最初の印を残す。並行に
    /// 付けても後の方は残らない）。付けられるかの判定は domain の [`LessonReview::mark_handled`] が済ませてある。
    /// 評価がなければ何もしない。
    fn save_handling(
        &self,
        review_id: LessonReviewId,
        handling: &LowRatedReviewHandling,
    ) -> impl Future<Output = Result<(), Unavailable>> + Send;

    /// コーチ `coach_id` への評価をすべて読む（数えない・並べない。並べるのは [`super::ListCoachReviews`]、数えるのは domain の
    /// [`domain::review::CoachReviewSummary::tally`]）。
    fn find_by_coach(
        &self,
        coach_id: AccountId,
    ) -> impl Future<Output = Result<Vec<LessonReview>, Unavailable>> + Send;

    /// 送った時刻が `since` 以降（含む）の評価をすべて読む（点で絞らない・並べない。低い評価を選んで並べるのは domain の
    /// [`domain::review::LowRatedReviewCriteria::select`]）。
    fn find_submitted_since(
        &self,
        since: SystemTime,
    ) -> impl Future<Output = Result<Vec<LessonReview>, Unavailable>> + Send;
}
