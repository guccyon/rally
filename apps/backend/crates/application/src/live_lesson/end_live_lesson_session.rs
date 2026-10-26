use domain::{
    DomainError,
    identity::AccountId,
    live_lesson::{LiveLessonSession, SessionId},
};

use super::{LiveLessonSessionRepository, UpdateSessionError, VideoCallProvider};
use crate::{
    Unavailable,
    analytics::{AnalyticsEvent, AnalyticsSink},
    clock::Clock,
};

/// 通話セッションの終了の入力。`account_id` は認証したアカウント（予約の生徒かコーチかは domain が確かめる）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndLiveLessonSessionCommand {
    pub account_id: AccountId,
    pub session_id: SessionId,
}

#[derive(Debug)]
pub enum EndLiveLessonSessionError {
    /// セッションがない
    SessionNotFound,
    /// 終了がドメインの不変条件を満たさない（予約の参加者でない、終了済み、終了が開始より前）
    Invalid(DomainError),
    /// 保存先・通話基盤の障害
    Unavailable(Unavailable),
}

/// 通話セッションの終了のユースケース（student-app の `endLiveLessonSession`）。
///
/// セッションを読んで終了できるかを domain で確かめ（[`LiveLessonSession::ensure_can_end`]）、通話基盤に
/// ルームを閉じさせて接続品質の生の指標を受け取り（[`VideoCallProvider::close_room`]）、
/// [`LiveLessonSession::end`] で記録して保存する。保存したら行動イベント `call.ended` を 4 つの指標とともに
/// 1 回発火する。通話品質スコアは計算しない（式はセマンティック層の 1 か所。docs/adr/0002 Q1）。
pub struct EndLiveLessonSession<S, V, A, C> {
    sessions: S,
    video: V,
    analytics: A,
    clock: C,
}

impl<S, V, A, C> EndLiveLessonSession<S, V, A, C>
where
    S: LiveLessonSessionRepository,
    V: VideoCallProvider,
    A: AnalyticsSink,
    C: Clock,
{
    pub fn new(sessions: S, video: V, analytics: A, clock: C) -> Self {
        Self {
            sessions,
            video,
            analytics,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: EndLiveLessonSessionCommand,
    ) -> Result<LiveLessonSession, EndLiveLessonSessionError> {
        let now = self.clock.now();
        self.sessions
            .find(command.session_id)
            .await
            .map_err(EndLiveLessonSessionError::Unavailable)?
            .ok_or(EndLiveLessonSessionError::SessionNotFound)?
            .ensure_can_end(command.account_id, now)
            .map_err(EndLiveLessonSessionError::Invalid)?;

        let quality = self
            .video
            .close_room(command.session_id)
            .await
            .map_err(EndLiveLessonSessionError::Unavailable)?;
        let session = self
            .sessions
            .update(command.session_id, |session| {
                session.end(command.account_id, now, quality)
            })
            .await
            .map_err(|error| match error {
                UpdateSessionError::NotFound => EndLiveLessonSessionError::SessionNotFound,
                UpdateSessionError::Rejected(error) => EndLiveLessonSessionError::Invalid(error),
                UpdateSessionError::Unavailable(error) => {
                    EndLiveLessonSessionError::Unavailable(error)
                }
            })?;

        let duration = session.duration().ok_or_else(|| {
            EndLiveLessonSessionError::Unavailable(Unavailable::new(
                "終了したセッションに終了時刻がない",
            ))
        })?;
        self.analytics
            .emit(AnalyticsEvent::CallEnded {
                student_id: session.student_id(),
                coach_id: session.coach_id(),
                booking_id: session.booking_id(),
                session_id: session.id(),
                duration_sec: duration.as_secs(),
                packet_loss_rate: quality.packet_loss_rate(),
                jitter_ms: quality.jitter_ms(),
                round_trip_ms: quality.round_trip_ms(),
                disconnect_count: quality.disconnect_count(),
                occurred_at: now,
            })
            .await;

        Ok(session)
    }

    pub fn sessions(&self) -> &S {
        &self.sessions
    }

    pub fn video(&self) -> &V {
        &self.video
    }

    pub fn analytics(&self) -> &A {
        &self.analytics
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        identity::Sport,
        live_lesson::{ConnectionQuality, SessionStatus},
        scheduling::{Booking, BookingId, Slot, SlotId, SlotStatus},
    };

    use super::*;
    use crate::{
        in_memory::{
            FixedClock, FixedVideoCallProvider, InMemoryLiveLessonSessionRepository,
            RecordingAnalyticsSink,
        },
        live_lesson::VideoCallProvider,
    };

    type UseCase = EndLiveLessonSession<
        InMemoryLiveLessonSessionRepository,
        FixedVideoCallProvider,
        RecordingAnalyticsSink,
        FixedClock,
    >;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const SESSION: SessionId = SessionId::from_u128(30);
    const STARTED_AT: SystemTime = SystemTime::UNIX_EPOCH;
    /// 通話を終える時刻（開始の 45 分 30 秒後）
    fn now() -> SystemTime {
        STARTED_AT + Duration::from_secs(45 * 60 + 30)
    }

    fn quality() -> ConnectionQuality {
        ConnectionQuality::new(4.5, 60, 320, 2).unwrap()
    }

    /// 確定した予約の通話を `STARTED_AT` に始め、ルームを開いた状態から始める。
    async fn use_case() -> UseCase {
        let mut slot = Slot::restore(
            SlotId::from_u128(10),
            COACH,
            Sport::Tennis,
            STARTED_AT,
            STARTED_AT + Duration::from_secs(3600),
            SlotStatus::Open,
        )
        .unwrap();
        let mut booking = Booking::reserve(
            BookingId::from_u128(20),
            STUDENT,
            false,
            &slot,
            &[],
            STARTED_AT,
        )
        .unwrap();
        booking.confirm(STUDENT, &mut slot, STARTED_AT).unwrap();
        let session =
            LiveLessonSession::start(SESSION, STUDENT, &booking, None, STARTED_AT).unwrap();
        let video = FixedVideoCallProvider::new(quality());
        video.create_room(SESSION, STUDENT).await.unwrap();
        EndLiveLessonSession::new(
            InMemoryLiveLessonSessionRepository::with_sessions([session]),
            video,
            RecordingAnalyticsSink::default(),
            FixedClock(now()),
        )
    }

    fn command(account_id: AccountId, session_id: SessionId) -> EndLiveLessonSessionCommand {
        EndLiveLessonSessionCommand {
            account_id,
            session_id,
        }
    }

    #[tokio::test]
    async fn ending_records_the_four_metrics_and_emits_call_ended() {
        let use_case = use_case().await;

        let session = use_case.execute(command(COACH, SESSION)).await.unwrap();

        assert_eq!(session.status(), SessionStatus::Ended);
        assert_eq!(session.ended_at(), Some(now()));
        assert_eq!(
            session.quality(),
            Some(quality()),
            "通話基盤の指標を記録する"
        );
        assert_eq!(use_case.sessions().all(), vec![session.clone()]);
        assert!(use_case.video().open_rooms().is_empty(), "ルームを閉じる");

        let analytics = use_case.analytics().events();
        assert_eq!(
            analytics,
            vec![AnalyticsEvent::CallEnded {
                student_id: STUDENT,
                coach_id: COACH,
                booking_id: BookingId::from_u128(20),
                session_id: SESSION,
                duration_sec: 45 * 60 + 30,
                packet_loss_rate: 4.5,
                jitter_ms: 60,
                round_trip_ms: 320,
                disconnect_count: 2,
                occurred_at: now(),
            }]
        );
        assert_eq!(analytics[0].name(), "call.ended");
    }

    #[tokio::test]
    async fn rejected_ends_change_nothing_and_emit_nothing() {
        let use_case = use_case().await;
        let before = use_case.sessions().all();

        let result = use_case
            .execute(command(STUDENT, SessionId::from_u128(99)))
            .await;
        assert!(
            matches!(result, Err(EndLiveLessonSessionError::SessionNotFound)),
            "{result:?}"
        );
        let result = use_case.execute(command(OTHER_STUDENT, SESSION)).await;
        assert!(
            matches!(
                result,
                Err(EndLiveLessonSessionError::Invalid(
                    DomainError::NotASessionParticipant
                ))
            ),
            "{result:?}"
        );
        assert_eq!(use_case.sessions().all(), before);
        assert_eq!(
            use_case.video().open_rooms(),
            vec![SESSION],
            "ルームは閉じない"
        );
        assert!(use_case.analytics().events().is_empty());

        // 2 回目の終了は拒否し、イベントは最初の 1 回だけ
        use_case.execute(command(STUDENT, SESSION)).await.unwrap();
        let result = use_case.execute(command(STUDENT, SESSION)).await;
        assert!(
            matches!(
                result,
                Err(EndLiveLessonSessionError::Invalid(
                    DomainError::LiveLessonSessionAlreadyEnded
                ))
            ),
            "{result:?}"
        );
        assert_eq!(use_case.analytics().events().len(), 1);
    }
}
