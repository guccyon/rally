use domain::{
    DomainError,
    identity::AccountId,
    live_lesson::{LiveLessonSession, SessionId},
};

use super::{LiveLessonSessionRepository, UpdateSessionError};
use crate::{Unavailable, clock::Clock};

/// 本部による通話セッションの終了の入力。`account_id` は認証した本部のアカウント（役割は境界が確かめる）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndLiveLessonSessionByHeadquartersCommand {
    pub account_id: AccountId,
    pub session_id: SessionId,
}

#[derive(Debug)]
pub enum EndLiveLessonSessionByHeadquartersError {
    /// セッションがない
    SessionNotFound,
    /// 終了がドメインの不変条件を満たさない（終了済み、終了が開始より前）
    Invalid(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// 終了の記録がないまま残った通話セッションを、本部が終わったことにするユースケース（admin-web の
/// `endLiveLessonSessionByHeadquarters`。本部だけが呼ぶ。D188）。
///
/// [`LiveLessonSession::end_by_headquarters`] で、終わった時刻を操作の時刻（[`Clock::now`]）にし、終わらせた本部の
/// アカウントを残して保存する。読み取りから保存までは保存先が直列にする（参加者の終了と並行しても片方だけが通る）。
///
/// 通話そのものは切らない（通話基盤のルームは閉じない）。通話基盤から指標を受け取らないので行動イベント `call.ended`
/// は発火せず（通話品質スコアの入力にしない）、境界間イベントも発行しない。ビデオ通話の暦月の枠は予約の確定と
/// キャンセルで数える（docs/adr/0005）ので、ここでは何も変えない。誰が呼べるか（本部だけ）は境界（interface）が確かめる。
pub struct EndLiveLessonSessionByHeadquarters<S, C> {
    sessions: S,
    clock: C,
}

impl<S, C> EndLiveLessonSessionByHeadquarters<S, C>
where
    S: LiveLessonSessionRepository,
    C: Clock,
{
    pub fn new(sessions: S, clock: C) -> Self {
        Self { sessions, clock }
    }

    pub async fn execute(
        &self,
        command: EndLiveLessonSessionByHeadquartersCommand,
    ) -> Result<LiveLessonSession, EndLiveLessonSessionByHeadquartersError> {
        let now = self.clock.now();
        self.sessions
            .update(command.session_id, |session| {
                session.end_by_headquarters(command.account_id, now)
            })
            .await
            .map_err(|error| match error {
                UpdateSessionError::NotFound => {
                    EndLiveLessonSessionByHeadquartersError::SessionNotFound
                }
                UpdateSessionError::Rejected(error) => {
                    EndLiveLessonSessionByHeadquartersError::Invalid(error)
                }
                UpdateSessionError::Unavailable(error) => {
                    EndLiveLessonSessionByHeadquartersError::Unavailable(error)
                }
            })
    }

    pub fn sessions(&self) -> &S {
        &self.sessions
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        live_lesson::{ConnectionQuality, SessionStatus},
        scheduling::BookingId,
    };

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryLiveLessonSessionRepository};

    const HOUR: Duration = Duration::from_secs(3600);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(7);
    const HEADQUARTERS: AccountId = AccountId::from_u128(9);
    const IN_PROGRESS: SessionId = SessionId::from_u128(30);
    const ENDED: SessionId = SessionId::from_u128(31);

    /// 2026-11-01T00:00:00Z（本部が操作する時刻）
    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200)
    }

    fn session(id: SessionId, ended_at: Option<SystemTime>) -> LiveLessonSession {
        LiveLessonSession::restore(
            id,
            BookingId::from_u128(100 + id.as_u128()),
            STUDENT,
            COACH,
            vec![],
            now() - 5 * HOUR,
            ended_at.map(|ended_at| (ended_at, ConnectionQuality::new(0.5, 12, 80, 0).unwrap())),
        )
        .unwrap()
    }

    fn use_case()
    -> EndLiveLessonSessionByHeadquarters<InMemoryLiveLessonSessionRepository, FixedClock> {
        EndLiveLessonSessionByHeadquarters::new(
            InMemoryLiveLessonSessionRepository::with_sessions([
                session(IN_PROGRESS, None),
                session(ENDED, Some(now() - 4 * HOUR)),
            ]),
            FixedClock(now()),
        )
    }

    fn command(session_id: SessionId) -> EndLiveLessonSessionByHeadquartersCommand {
        EndLiveLessonSessionByHeadquartersCommand {
            account_id: HEADQUARTERS,
            session_id,
        }
    }

    #[tokio::test]
    async fn headquarters_ends_a_session_left_in_progress_at_the_time_of_the_operation() {
        let use_case = use_case();

        let session = use_case.execute(command(IN_PROGRESS)).await.unwrap();

        assert_eq!(session.status(), SessionStatus::Ended);
        assert_eq!(session.ended_at(), Some(now()), "終わった時刻は操作の時刻");
        assert_eq!(session.ended_by_headquarters(), Some(HEADQUARTERS));
        assert_eq!(session.quality(), None);
        assert_eq!(
            use_case.sessions().all(),
            vec![session, session_ended_by_participants()],
            "保存する。ほかのセッションは変えない"
        );
    }

    fn session_ended_by_participants() -> LiveLessonSession {
        session(ENDED, Some(now() - 4 * HOUR))
    }

    #[tokio::test]
    async fn an_ended_session_or_a_missing_session_changes_nothing() {
        let use_case = use_case();
        let before = use_case.sessions().all();

        let result = use_case.execute(command(ENDED)).await;
        assert!(
            matches!(
                result,
                Err(EndLiveLessonSessionByHeadquartersError::Invalid(
                    DomainError::LiveLessonSessionAlreadyEnded
                ))
            ),
            "{result:?}"
        );
        let result = use_case.execute(command(SessionId::from_u128(99))).await;
        assert!(
            matches!(
                result,
                Err(EndLiveLessonSessionByHeadquartersError::SessionNotFound)
            ),
            "{result:?}"
        );
        assert_eq!(use_case.sessions().all(), before);
    }

    #[tokio::test]
    async fn the_second_end_is_rejected_and_keeps_the_first() {
        let use_case = use_case();
        let first = use_case.execute(command(IN_PROGRESS)).await.unwrap();

        let result = use_case
            .execute(EndLiveLessonSessionByHeadquartersCommand {
                account_id: AccountId::from_u128(10),
                session_id: IN_PROGRESS,
            })
            .await;
        assert!(
            matches!(
                result,
                Err(EndLiveLessonSessionByHeadquartersError::Invalid(
                    DomainError::LiveLessonSessionAlreadyEnded
                ))
            ),
            "{result:?}"
        );
        assert_eq!(
            use_case.sessions().all()[0],
            first,
            "最初の本部と時刻を残す"
        );
    }
}
