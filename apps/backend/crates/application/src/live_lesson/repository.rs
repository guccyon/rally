use std::future::Future;

use domain::{
    DomainError,
    live_lesson::{LiveLessonSession, SessionId},
    scheduling::BookingId,
};

use super::{InsertSessionError, UpdateSessionError};
use crate::Unavailable;

/// 通話セッションの保存先のポート。実装はテスト・ローカル起動用の
/// [`crate::in_memory::InMemoryLiveLessonSessionRepository`]（Postgres の `live_lesson.sessions` の実装は、
/// 予約の保存先を Postgres にするときに足す）。
///
/// 不変条件の判定は domain の [`LiveLessonSession`] が行う。保存先は「同じ予約のセッションは 1 つ」を
/// 保存の時点でも守る（並行な開始が両方保存されないように。DB では `sessions_booking_id_key`）。
pub trait LiveLessonSessionRepository: Send + Sync {
    /// 新しいセッションの ID を払い出す。
    fn next_id(&self) -> SessionId;

    /// `session_id` のセッション。なければ `None`。
    fn find(
        &self,
        session_id: SessionId,
    ) -> impl Future<Output = Result<Option<LiveLessonSession>, Unavailable>> + Send;

    /// `booking_id` の予約のセッション。なければ `None`。
    fn find_by_booking(
        &self,
        booking_id: BookingId,
    ) -> impl Future<Output = Result<Option<LiveLessonSession>, Unavailable>> + Send;

    /// 新しいセッションを保存する。同じ予約のセッションがすでにあれば
    /// [`InsertSessionError::BookingAlreadyHasSession`]。
    fn insert(
        &self,
        session: LiveLessonSession,
    ) -> impl Future<Output = Result<(), InsertSessionError>> + Send;

    /// `session_id` のセッションを読み、`update` に渡す。`update` が `Ok` なら保存して返す。
    /// 読み取りから保存までを直列にする（並行な終了が両方保存されないように）。
    fn update<F>(
        &self,
        session_id: SessionId,
        update: F,
    ) -> impl Future<Output = Result<LiveLessonSession, UpdateSessionError>> + Send
    where
        F: FnOnce(&mut LiveLessonSession) -> Result<(), DomainError> + Send;
}
