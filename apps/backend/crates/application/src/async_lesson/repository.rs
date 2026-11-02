use std::{future::Future, time::SystemTime};

use domain::{
    DomainError,
    async_lesson::{Video, VideoId, VideoReviewId},
    identity::AccountId,
};

use super::UpdateVideoError;
use crate::Unavailable;

/// 動画と添削の保存先のポート。実装はテスト・ローカル起動用の
/// [`crate::in_memory::InMemoryVideoRepository`]（Postgres の `async_lesson.videos` / `video_reviews` の実装は、
/// アカウントの保存先を Postgres にするときに足す）。
///
/// 不変条件の判定は domain の [`Video`] が行う。保存先は読み取りから保存までを直列にする（並行な返却が
/// 両方保存されないように。DB では `video_reviews_video_id_key`）。
pub trait VideoRepository: Send + Sync {
    /// 新しい動画の ID を払い出す。
    fn next_video_id(&self) -> VideoId;

    /// 新しい添削の ID を払い出す。
    fn next_review_id(&self) -> VideoReviewId;

    /// `video_id` の動画（返却した添削を含む）。なければ `None`。
    fn find(
        &self,
        video_id: VideoId,
    ) -> impl Future<Output = Result<Option<Video>, Unavailable>> + Send;

    /// `coach_id` のコーチに送られた動画（状態を問わない。返却した添削を含む）。並びは決めない。
    fn videos_of_coach(
        &self,
        coach_id: AccountId,
    ) -> impl Future<Output = Result<Vec<Video>, Unavailable>> + Send;

    /// `student_id` の生徒が送った動画（状態を問わない。返却した添削を含む）。並びは決めない。
    fn videos_of_student(
        &self,
        student_id: AccountId,
    ) -> impl Future<Output = Result<Vec<Video>, Unavailable>> + Send;

    /// コーチがまだ添削を返していない動画（すべてのコーチ・生徒。本部が見る返却待ちの一覧の材料）。並びは決めない。
    fn videos_awaiting_review(
        &self,
    ) -> impl Future<Output = Result<Vec<Video>, Unavailable>> + Send;

    /// 添削の返却の時刻が `starts_at`（含む）から `ends_at`（含まない）までの動画（すべてのコーチ・生徒。本部が見る
    /// 返した動画添削の記録の材料）。並びは決めない。
    fn videos_returned_between(
        &self,
        starts_at: SystemTime,
        ends_at: SystemTime,
    ) -> impl Future<Output = Result<Vec<Video>, Unavailable>> + Send;

    /// 送付した動画を保存する。
    fn insert(&self, video: Video) -> impl Future<Output = Result<(), Unavailable>> + Send;

    /// `video_id` の動画を読み、`update` に渡す。`update` が `Ok` なら保存して返す。
    fn update<F>(
        &self,
        video_id: VideoId,
        update: F,
    ) -> impl Future<Output = Result<Video, UpdateVideoError>> + Send
    where
        F: FnOnce(&mut Video) -> Result<(), DomainError> + Send;
}
