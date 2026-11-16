use domain::{
    identity::AccountId,
    live_lesson::{LiveLessonSession, SessionId},
};

use super::LiveLessonSessionRepository;
use crate::Unavailable;

/// 通話セッションの取得の入力。`account_id` は認証したアカウント（セッションの参加者かはここで確かめる）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetLiveLessonSessionQuery {
    pub account_id: AccountId,
    pub session_id: SessionId,
}

#[derive(Debug)]
pub enum GetLiveLessonSessionError {
    /// セッションがないか、`account_id` がセッションの参加者でない（参加者でないアカウントには、ないのと同じに見せる）
    SessionNotFound,
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// 通話セッションの取得のユースケース（student-app の `getLiveLessonSession`。Issue #1654）。
///
/// 入室した参加者が、相手の入室（[`LiveLessonSession::coach_joined_at`]・[`LiveLessonSession::joined_at`]）や終了を
/// 取り直して知るのに使う。読むだけで、入室の記録・`call.started` の発火・通話への参加はしない（入室は
/// [`super::StartLiveLessonSession`]）。読めるのはセッションの参加者（コーチと、結び付いた予約の生徒。
/// [`LiveLessonSession::is_participant`]）だけで、それ以外のアカウントにはセッションがないのと同じ
/// [`GetLiveLessonSessionError::SessionNotFound`] を返す。
pub struct GetLiveLessonSession<S> {
    sessions: S,
}

impl<S> GetLiveLessonSession<S>
where
    S: LiveLessonSessionRepository,
{
    pub fn new(sessions: S) -> Self {
        Self { sessions }
    }

    pub async fn execute(
        &self,
        query: GetLiveLessonSessionQuery,
    ) -> Result<LiveLessonSession, GetLiveLessonSessionError> {
        self.sessions
            .find(query.session_id)
            .await
            .map_err(GetLiveLessonSessionError::Unavailable)?
            .filter(|session| session.is_participant(query.account_id))
            .ok_or(GetLiveLessonSessionError::SessionNotFound)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::Sport,
        live_lesson::ConnectionQuality,
        scheduling::{Booking, BookingId, Slot, SlotId, SlotStatus},
    };

    use super::*;
    use crate::in_memory::InMemoryLiveLessonSessionRepository;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const SESSION: SessionId = SessionId::from_u128(30);
    const STARTED_AT: SystemTime = SystemTime::UNIX_EPOCH;

    /// 生徒が確定した予約の通話を `STARTED_AT` に始めた状態から始める（コーチはまだ入室していない）。
    fn use_case() -> (
        GetLiveLessonSession<InMemoryLiveLessonSessionRepository>,
        Booking,
    ) {
        let mut slot = Slot::restore(
            SlotId::from_u128(10),
            COACH,
            Sport::Tennis,
            STARTED_AT,
            STARTED_AT + Duration::from_secs(3600),
            SlotStatus::Open,
            domain::scheduling::SlotSeats::default(),
        )
        .unwrap();
        let mut booking = Booking::reserve(
            BookingId::from_u128(20),
            STUDENT,
            false,
            &mut slot,
            &[],
            STARTED_AT,
        )
        .unwrap();
        booking.confirm(STUDENT, &mut slot, STARTED_AT).unwrap();
        let session =
            LiveLessonSession::start(SESSION, STUDENT, &booking, &[], None, STARTED_AT).unwrap();
        (
            GetLiveLessonSession::new(InMemoryLiveLessonSessionRepository::with_sessions([
                session,
            ])),
            booking,
        )
    }

    fn query(account_id: AccountId, session_id: SessionId) -> GetLiveLessonSessionQuery {
        GetLiveLessonSessionQuery {
            account_id,
            session_id,
        }
    }

    #[tokio::test]
    async fn participants_read_whether_the_coach_has_joined() {
        let (use_case, booking) = use_case();
        for account in [STUDENT, COACH] {
            let session = use_case.execute(query(account, SESSION)).await.unwrap();
            assert_eq!(session.id(), SESSION);
            assert_eq!(session.coach_joined_at(), None);
            assert_eq!(session.joined_at(STUDENT), Some(STARTED_AT));
        }

        let joined_at = STARTED_AT + Duration::from_secs(90);
        use_case
            .sessions
            .update(SESSION, |session| session.join(COACH, &booking, joined_at))
            .await
            .unwrap();
        let session = use_case.execute(query(STUDENT, SESSION)).await.unwrap();
        assert_eq!(session.coach_joined_at(), Some(joined_at));
    }

    #[tokio::test]
    async fn an_ended_session_is_read_by_participants() {
        let (use_case, _) = use_case();
        use_case
            .sessions
            .update(SESSION, |session| {
                session.end(
                    COACH,
                    STARTED_AT + Duration::from_secs(600),
                    ConnectionQuality::new(0.5, 12, 80, 0).unwrap(),
                )
            })
            .await
            .unwrap();
        let session = use_case.execute(query(STUDENT, SESSION)).await.unwrap();
        assert_eq!(
            session.ended_at(),
            Some(STARTED_AT + Duration::from_secs(600))
        );
    }

    #[tokio::test]
    async fn others_and_unknown_sessions_are_not_found() {
        let (use_case, _) = use_case();
        for (account, session) in [
            (OTHER_STUDENT, SESSION),
            (STUDENT, SessionId::from_u128(99)),
        ] {
            let result = use_case.execute(query(account, session)).await;
            assert!(
                matches!(result, Err(GetLiveLessonSessionError::SessionNotFound)),
                "{result:?}"
            );
        }
    }
}
