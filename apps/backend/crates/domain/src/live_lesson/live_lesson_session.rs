use std::time::{Duration, SystemTime};

use super::{ConnectionQuality, SessionId, SessionStatus};
use crate::{
    DomainError,
    identity::AccountId,
    lesson::{LessonCompletion, LessonId, LessonSource},
    scheduling::{Booking, BookingId, BookingStatus},
};

/// 同期レッスン（glossary の live_lesson）の通話セッション。確定した予約 1 件につき 1 つ。
///
/// 開始（[`LiveLessonSession::start`]）した直後は [`SessionStatus::InProgress`] で、終了
/// （[`LiveLessonSession::end`]）すると接続品質の生の指標（[`ConnectionQuality`]）を持つ
/// [`SessionStatus::Ended`] になる。生徒・コーチは予約のもの。開始・終了できるのはその 2 人（参加者）だけ。
#[derive(Debug, Clone, PartialEq)]
pub struct LiveLessonSession {
    id: SessionId,
    booking_id: BookingId,
    student_id: AccountId,
    coach_id: AccountId,
    started_at: SystemTime,
    ended_at: Option<SystemTime>,
    quality: Option<ConnectionQuality>,
}

impl LiveLessonSession {
    /// 予約の参加者 `participant` が `booking` の通話を `now` に始める。
    ///
    /// `existing` はその予約の既存のセッション（なければ `None`）。次の不変条件を破れば [`DomainError`] を返す。
    ///
    /// - 開始できるのは予約の生徒かコーチだけ（[`DomainError::NotASessionParticipant`]）
    /// - 確定した予約にしか開始できない（[`DomainError::BookingNotConfirmed`]）
    /// - 同じ予約のセッションは 1 つ（[`DomainError::LiveLessonSessionAlreadyStarted`]）
    pub fn start(
        id: SessionId,
        participant: AccountId,
        booking: &Booking,
        existing: Option<&LiveLessonSession>,
        now: SystemTime,
    ) -> Result<Self, DomainError> {
        if participant != booking.student_id() && participant != booking.coach_id() {
            return Err(DomainError::NotASessionParticipant);
        }
        if booking.status() != BookingStatus::Confirmed {
            return Err(DomainError::BookingNotConfirmed {
                status: booking.status(),
            });
        }
        if existing.is_some_and(|session| session.booking_id == booking.id()) {
            return Err(DomainError::LiveLessonSessionAlreadyStarted);
        }
        Ok(Self {
            id,
            booking_id: booking.id(),
            student_id: booking.student_id(),
            coach_id: booking.coach_id(),
            started_at: now,
            ended_at: None,
            quality: None,
        })
    }

    /// 保存先から読んだセッションを組み立てる（保存先の実装用）。`ended` は終了した時刻と、そのときに記録した
    /// 接続品質（終了するまでは `None`）。終了が開始より前なら [`DomainError::SessionEndsBeforeItStarts`]。
    pub fn restore(
        id: SessionId,
        booking_id: BookingId,
        student_id: AccountId,
        coach_id: AccountId,
        started_at: SystemTime,
        ended: Option<(SystemTime, ConnectionQuality)>,
    ) -> Result<Self, DomainError> {
        if ended.is_some_and(|(ended_at, _)| ended_at < started_at) {
            return Err(DomainError::SessionEndsBeforeItStarts);
        }
        Ok(Self {
            id,
            booking_id,
            student_id,
            coach_id,
            started_at,
            ended_at: ended.map(|(ended_at, _)| ended_at),
            quality: ended.map(|(_, quality)| quality),
        })
    }

    /// 参加者 `participant` が `now` にセッションを終了できるかを確かめる（[`LiveLessonSession::end`] と同じ判定）。
    ///
    /// 参加者でなければ [`DomainError::NotASessionParticipant`]、終了済みなら
    /// [`DomainError::LiveLessonSessionAlreadyEnded`]、`now` が開始より前なら
    /// [`DomainError::SessionEndsBeforeItStarts`]（開始と同じ時刻は終了できる。通話の長さは 0 秒）。
    pub fn ensure_can_end(
        &self,
        participant: AccountId,
        now: SystemTime,
    ) -> Result<(), DomainError> {
        if !self.is_participant(participant) {
            return Err(DomainError::NotASessionParticipant);
        }
        if self.ended_at.is_some() {
            return Err(DomainError::LiveLessonSessionAlreadyEnded);
        }
        if now < self.started_at {
            return Err(DomainError::SessionEndsBeforeItStarts);
        }
        Ok(())
    }

    /// 参加者 `participant` が `now` にセッションを終了し、接続品質の指標 `quality` を記録する。
    ///
    /// 判定は [`LiveLessonSession::ensure_can_end`]。失敗したときはセッションを変えない。
    pub fn end(
        &mut self,
        participant: AccountId,
        now: SystemTime,
        quality: ConnectionQuality,
    ) -> Result<(), DomainError> {
        self.ensure_can_end(participant, now)?;
        self.ended_at = Some(now);
        self.quality = Some(quality);
        Ok(())
    }

    /// 参加者 `participant` が `now` に、このセッションの同期レッスンの完了を記録する（完了記録の ID は `lesson_id`）。
    ///
    /// `history` はその生徒の既存の完了記録（同期・非同期の両方。`is_first_lesson` の判定と重複の検出に使う。
    /// [`LessonCompletion`]）。次の不変条件を破れば [`DomainError`] を返す。セッションは変えない。
    ///
    /// - 記録できるのは予約の生徒かコーチだけ（[`DomainError::NotASessionParticipant`]）
    /// - 終了したセッションだけ（[`DomainError::LiveLessonSessionNotEnded`]）
    /// - 記録は終了の後（同じ時刻は許す。[`DomainError::CompletionBeforeLessonEnded`]）
    /// - 同じセッションの完了は 1 回（[`DomainError::LessonAlreadyCompleted`]）
    pub fn complete(
        &self,
        lesson_id: LessonId,
        participant: AccountId,
        history: &[LessonCompletion],
        now: SystemTime,
    ) -> Result<LessonCompletion, DomainError> {
        if !self.is_participant(participant) {
            return Err(DomainError::NotASessionParticipant);
        }
        let ended_at = self
            .ended_at
            .ok_or(DomainError::LiveLessonSessionNotEnded)?;
        if now < ended_at {
            return Err(DomainError::CompletionBeforeLessonEnded);
        }
        LessonCompletion::record(
            lesson_id,
            self.student_id,
            self.coach_id,
            LessonSource::Live(self.id),
            history,
            now,
        )
    }

    /// 予約の生徒かコーチか。
    pub fn is_participant(&self, account: AccountId) -> bool {
        account == self.student_id || account == self.coach_id
    }

    pub const fn id(&self) -> SessionId {
        self.id
    }

    pub const fn booking_id(&self) -> BookingId {
        self.booking_id
    }

    pub const fn student_id(&self) -> AccountId {
        self.student_id
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    pub const fn status(&self) -> SessionStatus {
        match self.ended_at {
            None => SessionStatus::InProgress,
            Some(_) => SessionStatus::Ended,
        }
    }

    pub const fn started_at(&self) -> SystemTime {
        self.started_at
    }

    /// 終了した時刻。終了するまでは `None`。
    pub const fn ended_at(&self) -> Option<SystemTime> {
        self.ended_at
    }

    /// 通話の長さ（開始から終了まで）。終了するまでは `None`。
    pub fn duration(&self) -> Option<Duration> {
        self.ended_at.map(|ended_at| {
            ended_at
                .duration_since(self.started_at)
                .expect("終了は開始より前にならない（ensure_can_end）")
        })
    }

    /// 終了時に記録した接続品質の指標。終了するまでは `None`。
    pub const fn quality(&self) -> Option<ConnectionQuality> {
        self.quality
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        identity::Sport,
        scheduling::{Slot, SlotId, SlotStatus},
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(3);
    const NOW: SystemTime = SystemTime::UNIX_EPOCH;
    const SESSION: SessionId = SessionId::from_u128(50);

    fn at(minute: u64) -> SystemTime {
        NOW + Duration::from_secs(minute * 60)
    }

    /// 生徒が予約した直後（確定前）の予約と、そのスロット。
    fn pending() -> (Booking, Slot) {
        let mut slot = Slot::restore(
            SlotId::from_u128(10),
            COACH,
            Sport::Tennis,
            at(600),
            at(660),
            SlotStatus::Open,
            crate::scheduling::SlotSeats::default(),
        )
        .unwrap();
        let booking =
            Booking::reserve(BookingId::from_u128(20), STUDENT, true, &mut slot, &[], NOW).unwrap();
        (booking, slot)
    }

    fn confirmed() -> Booking {
        let (mut booking, mut slot) = pending();
        booking.confirm(STUDENT, &mut slot, NOW).unwrap();
        booking
    }

    fn quality() -> ConnectionQuality {
        ConnectionQuality::new(0.5, 12, 80, 0).unwrap()
    }

    #[test]
    fn participants_start_a_session_of_a_confirmed_booking() {
        let booking = confirmed();
        for participant in [STUDENT, COACH] {
            let session =
                LiveLessonSession::start(SESSION, participant, &booking, None, at(600)).unwrap();
            assert_eq!(session.id(), SESSION);
            assert_eq!(session.booking_id(), booking.id());
            assert_eq!(session.student_id(), STUDENT);
            assert_eq!(session.coach_id(), COACH);
            assert_eq!(session.status(), SessionStatus::InProgress);
            assert_eq!(session.started_at(), at(600));
            assert_eq!(session.ended_at(), None);
            assert_eq!(session.duration(), None);
            assert_eq!(session.quality(), None);
        }
    }

    #[test]
    fn only_confirmed_bookings_can_be_started() {
        let (pending, mut slot) = pending();
        assert_eq!(
            LiveLessonSession::start(SESSION, STUDENT, &pending, None, at(600)),
            Err(DomainError::BookingNotConfirmed {
                status: BookingStatus::Pending
            })
        );
        let mut cancelled = pending.clone();
        cancelled.cancel(STUDENT, &mut slot, NOW).unwrap();
        assert_eq!(
            LiveLessonSession::start(SESSION, STUDENT, &cancelled, None, at(600)),
            Err(DomainError::BookingNotConfirmed {
                status: BookingStatus::Cancelled
            })
        );
    }

    #[test]
    fn only_participants_can_start() {
        // 参加者でないことを予約の状態より先に返す（予約があることを知らせない）
        let (pending, _) = pending();
        for booking in [confirmed(), pending] {
            assert_eq!(
                LiveLessonSession::start(SESSION, OTHER_STUDENT, &booking, None, at(600)),
                Err(DomainError::NotASessionParticipant)
            );
        }
    }

    #[test]
    fn a_booking_has_at_most_one_session() {
        let booking = confirmed();
        let first = LiveLessonSession::start(SESSION, STUDENT, &booking, None, at(600)).unwrap();
        let mut ended = first.clone();
        ended.end(STUDENT, at(650), quality()).unwrap();
        for existing in [first, ended] {
            assert_eq!(
                LiveLessonSession::start(
                    SessionId::from_u128(51),
                    COACH,
                    &booking,
                    Some(&existing),
                    at(700)
                ),
                Err(DomainError::LiveLessonSessionAlreadyStarted)
            );
        }
    }

    #[test]
    fn ending_records_the_connection_quality() {
        let booking = confirmed();
        for participant in [STUDENT, COACH] {
            let mut session =
                LiveLessonSession::start(SESSION, STUDENT, &booking, None, at(600)).unwrap();
            session.end(participant, at(645), quality()).unwrap();
            assert_eq!(session.status(), SessionStatus::Ended);
            assert_eq!(session.ended_at(), Some(at(645)));
            assert_eq!(session.duration(), Some(Duration::from_secs(45 * 60)));
            assert_eq!(session.quality(), Some(quality()));
        }
    }

    #[test]
    fn a_session_can_end_at_the_moment_it_started() {
        let mut session =
            LiveLessonSession::start(SESSION, STUDENT, &confirmed(), None, at(600)).unwrap();
        session.end(STUDENT, at(600), quality()).unwrap();
        assert_eq!(session.duration(), Some(Duration::ZERO));
    }

    #[test]
    fn end_failures_change_nothing() {
        let started =
            LiveLessonSession::start(SESSION, STUDENT, &confirmed(), None, at(600)).unwrap();
        let mut ended = started.clone();
        ended.end(STUDENT, at(650), quality()).unwrap();

        for (mut session, participant, now, expected) in [
            (
                started.clone(),
                OTHER_STUDENT,
                at(650),
                DomainError::NotASessionParticipant,
            ),
            (
                started.clone(),
                STUDENT,
                at(599),
                DomainError::SessionEndsBeforeItStarts,
            ),
            (
                ended.clone(),
                COACH,
                at(700),
                DomainError::LiveLessonSessionAlreadyEnded,
            ),
        ] {
            let before = session.clone();
            assert_eq!(
                session.ensure_can_end(participant, now),
                Err(expected.clone())
            );
            assert_eq!(session.end(participant, now, quality()), Err(expected));
            assert_eq!(session, before);
        }
    }

    const LESSON: LessonId = LessonId::from_u128(70);

    fn ended() -> LiveLessonSession {
        let mut session =
            LiveLessonSession::start(SESSION, STUDENT, &confirmed(), None, at(600)).unwrap();
        session.end(COACH, at(650), quality()).unwrap();
        session
    }

    #[test]
    fn participants_complete_an_ended_session() {
        let session = ended();
        for participant in [STUDENT, COACH] {
            let completion = session.complete(LESSON, participant, &[], at(651)).unwrap();
            assert_eq!(completion.id(), LESSON);
            assert_eq!(
                (completion.student_id(), completion.coach_id()),
                (STUDENT, COACH)
            );
            assert_eq!(completion.source(), LessonSource::Live(SESSION));
            assert_eq!(completion.lesson_type(), crate::lesson::LessonType::Live);
            assert_eq!(completion.completed_at(), at(651));
            assert!(completion.is_first_lesson());
        }
        // 終了と同じ時刻にも記録できる
        session.complete(LESSON, STUDENT, &[], at(650)).unwrap();
    }

    #[test]
    fn completion_failures() {
        let started =
            LiveLessonSession::start(SESSION, STUDENT, &confirmed(), None, at(600)).unwrap();
        let done = ended().complete(LESSON, STUDENT, &[], at(651)).unwrap();
        for (session, participant, history, now, expected) in [
            (
                ended(),
                OTHER_STUDENT,
                vec![],
                at(651),
                DomainError::NotASessionParticipant,
            ),
            (
                started,
                STUDENT,
                vec![],
                at(651),
                DomainError::LiveLessonSessionNotEnded,
            ),
            (
                ended(),
                STUDENT,
                vec![],
                at(649),
                DomainError::CompletionBeforeLessonEnded,
            ),
            (
                ended(),
                COACH,
                vec![done],
                at(700),
                DomainError::LessonAlreadyCompleted,
            ),
        ] {
            assert_eq!(
                session.complete(LessonId::from_u128(71), participant, &history, now),
                Err(expected)
            );
        }
    }

    #[test]
    fn restore_round_trips_an_ended_session_and_rejects_ending_before_start() {
        let mut session =
            LiveLessonSession::start(SESSION, STUDENT, &confirmed(), None, at(600)).unwrap();
        session.end(COACH, at(650), quality()).unwrap();

        let restored = LiveLessonSession::restore(
            SESSION,
            session.booking_id(),
            STUDENT,
            COACH,
            at(600),
            Some((at(650), quality())),
        )
        .unwrap();
        assert_eq!(restored, session);

        assert_eq!(
            LiveLessonSession::restore(
                SESSION,
                session.booking_id(),
                STUDENT,
                COACH,
                at(600),
                Some((at(599), quality())),
            ),
            Err(DomainError::SessionEndsBeforeItStarts)
        );
    }

    #[test]
    fn restore_accepts_ending_at_or_after_the_start() {
        let restore = |ended_at| {
            LiveLessonSession::restore(
                SESSION,
                BookingId::from_u128(20),
                STUDENT,
                COACH,
                at(600),
                Some((ended_at, quality())),
            )
        };
        // 開始の 1 分前は断る
        assert_eq!(
            restore(at(599)),
            Err(DomainError::SessionEndsBeforeItStarts)
        );
        // 開始と同じ時刻（通話の長さ 0 秒）と、その 1 分後は読める
        let same = restore(at(600)).unwrap();
        assert_eq!(same.ended_at(), Some(at(600)));
        assert_eq!(same.duration(), Some(Duration::ZERO));
        let later = restore(at(601)).unwrap();
        assert_eq!(later.ended_at(), Some(at(601)));
        assert_eq!(later.duration(), Some(Duration::from_secs(60)));
    }
}
