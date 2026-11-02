use std::{future::Future, time::SystemTime};

use domain::{
    DomainError,
    identity::AccountId,
    live_lesson::{LiveLessonSession, SessionId},
    scheduling::BookingId,
};

use super::{InsertSessionError, UpdateSessionError};
use crate::Unavailable;

/// 通話セッションの保存先のポート。実装はテスト・ローカル起動用の
/// [`crate::in_memory::InMemoryLiveLessonSessionRepository`]（Postgres の `live_lesson.sessions` の実装は、
/// 予約の保存先を Postgres にするときに足す）。
///
/// 不変条件の判定は domain の [`LiveLessonSession`] が行う。保存先は「同じ予約のセッションは 1 つ」を、
/// セッションに結び付いた予約（[`LiveLessonSession::bookings`]。グループレッスンでは人数分）のすべてについて
/// 保存の時点でも守る（並行な開始が両方保存されないように。DB では `sessions_booking_id_key` と
/// `session_bookings_booking_id_key`）。
pub trait LiveLessonSessionRepository: Send + Sync {
    /// 新しいセッションの ID を払い出す。
    fn next_id(&self) -> SessionId;

    /// `session_id` のセッション。なければ `None`。
    fn find(
        &self,
        session_id: SessionId,
    ) -> impl Future<Output = Result<Option<LiveLessonSession>, Unavailable>> + Send;

    /// `booking_id` の予約が結び付いたセッション（開始した予約でなくてもよい）。なければ `None`。
    fn find_by_booking(
        &self,
        booking_id: BookingId,
    ) -> impl Future<Output = Result<Option<LiveLessonSession>, Unavailable>> + Send;

    /// 終了の記録がなく、`started_at_or_before`（含む）までに開始したセッション（本部が見る、終わっていないまま時間の
    /// 過ぎたセッションの一覧）。並びは問わない（選び方と並べ方は domain の
    /// [`domain::live_lesson::OverdueSessionCriteria`]）。
    fn find_in_progress_started_at_or_before(
        &self,
        started_at_or_before: SystemTime,
    ) -> impl Future<Output = Result<Vec<LiveLessonSession>, Unavailable>> + Send;

    /// `starts_at`（含む）から `ends_at`（含まない）までに開始したセッション（終了の有無を問わない。本部が見る通話
    /// セッションの記録の一覧）。`coach_id` があればそのコーチの、`student_id` があればその生徒の予約が結び付いた
    /// （グループレッスンでは開始した予約の生徒に限らない）セッションだけ。並びは問わない（選び方と並べ方は domain の
    /// [`domain::live_lesson::SessionHistoryCriteria`]）。
    fn find_started_between(
        &self,
        starts_at: SystemTime,
        ends_at: SystemTime,
        coach_id: Option<AccountId>,
        student_id: Option<AccountId>,
    ) -> impl Future<Output = Result<Vec<LiveLessonSession>, Unavailable>> + Send;

    /// 新しいセッションを、結び付いた予約とともに保存する。結び付いた予約のどれかのセッションがすでにあれば
    /// [`InsertSessionError::BookingAlreadyHasSession`]（何も保存しない）。
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
