use domain::{
    DomainError, identity::AccountId, live_lesson::LiveLessonSession, scheduling::BookingId,
};

use super::{BookingLookup, InsertSessionError, LiveLessonSessionRepository, VideoCallProvider};
use crate::{
    Unavailable,
    analytics::{AnalyticsEvent, AnalyticsSink},
    clock::Clock,
};

/// 通話セッションの開始の入力。`account_id` は認証したアカウント（予約の生徒かコーチかは domain が確かめる）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartLiveLessonSessionCommand {
    pub account_id: AccountId,
    pub booking_id: BookingId,
}

/// 開始したセッションと、開始したアカウントが通話ルームに参加するためのトークン（通話基盤が発行する）。
#[derive(Debug, Clone, PartialEq)]
pub struct StartedLiveLessonSession {
    pub session: LiveLessonSession,
    pub room_token: String,
}

#[derive(Debug)]
pub enum StartLiveLessonSessionError {
    /// 予約がない
    BookingNotFound,
    /// 開始がドメインの不変条件を満たさない（予約の参加者でない、確定した予約でない、
    /// その予約のセッションがすでにある）
    Invalid(DomainError),
    /// 保存先・通話基盤の障害
    Unavailable(Unavailable),
}

/// 通話セッションの開始のユースケース（student-app の `startLiveLessonSession`）。
///
/// 予約を scheduling に問い合わせ（[`BookingLookup`]）、domain の [`LiveLessonSession::start`] で開始して
/// から通話基盤にルームを作らせ（[`VideoCallProvider`]）、セッションを保存する。保存したら行動イベント
/// `call.started` を 1 回発火する。拒否・失敗では発火しない。
///
/// 同じ予約の開始が並行して両方 domain の判定を通っても、保存先が後の方を拒む（そのとき作ったルームは閉じる）。
pub struct StartLiveLessonSession<S, B, V, A, C> {
    sessions: S,
    bookings: B,
    video: V,
    analytics: A,
    clock: C,
}

impl<S, B, V, A, C> StartLiveLessonSession<S, B, V, A, C>
where
    S: LiveLessonSessionRepository,
    B: BookingLookup,
    V: VideoCallProvider,
    A: AnalyticsSink,
    C: Clock,
{
    pub fn new(sessions: S, bookings: B, video: V, analytics: A, clock: C) -> Self {
        Self {
            sessions,
            bookings,
            video,
            analytics,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: StartLiveLessonSessionCommand,
    ) -> Result<StartedLiveLessonSession, StartLiveLessonSessionError> {
        let now = self.clock.now();
        let booking = self
            .bookings
            .find_booking(command.booking_id)
            .await
            .map_err(StartLiveLessonSessionError::Unavailable)?
            .ok_or(StartLiveLessonSessionError::BookingNotFound)?;
        let existing = self
            .sessions
            .find_by_booking(command.booking_id)
            .await
            .map_err(StartLiveLessonSessionError::Unavailable)?;
        let session = LiveLessonSession::start(
            self.sessions.next_id(),
            command.account_id,
            &booking,
            existing.as_ref(),
            now,
        )
        .map_err(StartLiveLessonSessionError::Invalid)?;

        let room_token = self
            .video
            .create_room(session.id(), command.account_id)
            .await
            .map_err(StartLiveLessonSessionError::Unavailable)?;
        if let Err(error) = self.sessions.insert(session.clone()).await {
            // 保存できなかったセッションのルームは使われないので閉じる（閉じられなくても結果は変えない）
            let _ = self.video.close_room(session.id()).await;
            return Err(match error {
                InsertSessionError::BookingAlreadyHasSession => {
                    StartLiveLessonSessionError::Invalid(
                        DomainError::LiveLessonSessionAlreadyStarted,
                    )
                }
                InsertSessionError::Unavailable(error) => {
                    StartLiveLessonSessionError::Unavailable(error)
                }
            });
        }

        self.analytics
            .emit(AnalyticsEvent::CallStarted {
                student_id: session.student_id(),
                coach_id: session.coach_id(),
                booking_id: session.booking_id(),
                session_id: session.id(),
                occurred_at: now,
            })
            .await;

        Ok(StartedLiveLessonSession {
            session,
            room_token,
        })
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
        live_lesson::SessionStatus,
        scheduling::{Booking, BookingStatus, Slot, SlotId, SlotStatus},
    };

    use super::*;
    use crate::{
        in_memory::{
            FixedClock, FixedVideoCallProvider, InMemoryBookingRepository,
            InMemoryLiveLessonSessionRepository, InMemorySlotRepository, RecordingAnalyticsSink,
        },
        scheduling::{BookingRepository, SchedulingBookingLookup},
    };

    type UseCase = StartLiveLessonSession<
        InMemoryLiveLessonSessionRepository,
        SchedulingBookingLookup<InMemoryBookingRepository>,
        FixedVideoCallProvider,
        RecordingAnalyticsSink,
        FixedClock,
    >;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const BOOKED_AT: SystemTime = SystemTime::UNIX_EPOCH;
    /// 通話を始める時刻（スロットの開始と同じ）
    fn now() -> SystemTime {
        BOOKED_AT + Duration::from_secs(36_000)
    }

    /// スロット 10（公開中）を生徒が予約した状態から始める。`confirm` なら確定まで進める。
    async fn use_case(confirm: bool) -> (UseCase, Booking) {
        let slots = InMemorySlotRepository::with_slots([Slot::restore(
            SlotId::from_u128(10),
            COACH,
            Sport::Tennis,
            now(),
            now() + Duration::from_secs(3600),
            SlotStatus::Open,
            domain::scheduling::SlotSeats::default(),
        )
        .unwrap()]);
        let bookings = InMemoryBookingRepository::new(slots);
        let mut booking = bookings
            .reserve(SlotId::from_u128(10), STUDENT, |slot, existing| {
                Booking::reserve(
                    BookingId::from_u128(20),
                    STUDENT,
                    true,
                    slot,
                    existing,
                    BOOKED_AT,
                )
            })
            .await
            .unwrap();
        if confirm {
            booking = bookings
                .update(booking.id(), |booking, slot| {
                    booking.confirm(STUDENT, slot, BOOKED_AT)
                })
                .await
                .unwrap();
        }
        let use_case = StartLiveLessonSession::new(
            InMemoryLiveLessonSessionRepository::default(),
            SchedulingBookingLookup::new(bookings),
            FixedVideoCallProvider::default(),
            RecordingAnalyticsSink::default(),
            FixedClock(now()),
        );
        (use_case, booking)
    }

    fn command(account_id: AccountId, booking_id: BookingId) -> StartLiveLessonSessionCommand {
        StartLiveLessonSessionCommand {
            account_id,
            booking_id,
        }
    }

    #[tokio::test]
    async fn starting_saves_the_session_opens_a_room_and_emits_call_started() {
        let (use_case, booking) = use_case(true).await;

        let started = use_case
            .execute(command(STUDENT, booking.id()))
            .await
            .unwrap();

        let session = &started.session;
        assert_eq!(session.status(), SessionStatus::InProgress);
        assert_eq!(
            (
                session.booking_id(),
                session.student_id(),
                session.coach_id()
            ),
            (booking.id(), STUDENT, COACH)
        );
        assert_eq!(session.started_at(), now());
        assert_eq!(use_case.sessions().all(), vec![session.clone()]);
        assert_eq!(use_case.video().open_rooms(), vec![session.id()]);
        assert!(!started.room_token.is_empty());

        let analytics = use_case.analytics().events();
        assert_eq!(
            analytics,
            vec![AnalyticsEvent::CallStarted {
                student_id: STUDENT,
                coach_id: COACH,
                booking_id: booking.id(),
                session_id: session.id(),
                occurred_at: now(),
            }]
        );
        assert_eq!(analytics[0].name(), "call.started");
    }

    #[tokio::test]
    async fn a_booking_that_is_not_confirmed_cannot_be_started() {
        let (use_case, pending) = use_case(false).await;
        assert_eq!(pending.status(), BookingStatus::Pending);

        let result = use_case.execute(command(STUDENT, pending.id())).await;

        assert!(
            matches!(
                result,
                Err(StartLiveLessonSessionError::Invalid(
                    DomainError::BookingNotConfirmed {
                        status: BookingStatus::Pending
                    }
                ))
            ),
            "{result:?}"
        );
        assert!(use_case.sessions().all().is_empty());
        assert!(use_case.video().open_rooms().is_empty(), "ルームは作らない");
        assert!(use_case.analytics().events().is_empty());
    }

    #[tokio::test]
    async fn rejected_starts_change_nothing_and_emit_nothing() {
        let (use_case, booking) = use_case(true).await;

        let result = use_case
            .execute(command(STUDENT, BookingId::from_u128(99)))
            .await;
        assert!(
            matches!(result, Err(StartLiveLessonSessionError::BookingNotFound)),
            "{result:?}"
        );
        let result = use_case.execute(command(OTHER_STUDENT, booking.id())).await;
        assert!(
            matches!(
                result,
                Err(StartLiveLessonSessionError::Invalid(
                    DomainError::NotASessionParticipant
                ))
            ),
            "{result:?}"
        );
        assert!(use_case.sessions().all().is_empty());
        assert!(use_case.analytics().events().is_empty());

        // 同じ予約の 2 回目の開始は、コーチからでも拒否する
        use_case
            .execute(command(STUDENT, booking.id()))
            .await
            .unwrap();
        let result = use_case.execute(command(COACH, booking.id())).await;
        assert!(
            matches!(
                result,
                Err(StartLiveLessonSessionError::Invalid(
                    DomainError::LiveLessonSessionAlreadyStarted
                ))
            ),
            "{result:?}"
        );
        assert_eq!(use_case.sessions().all().len(), 1);
        assert_eq!(use_case.video().open_rooms().len(), 1);
        assert_eq!(use_case.analytics().events().len(), 1);
    }
}
