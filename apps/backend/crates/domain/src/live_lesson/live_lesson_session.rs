use std::time::{Duration, SystemTime};

use super::{ConnectionQuality, SessionBooking, SessionId, SessionStatus};
use crate::{
    DomainError,
    identity::AccountId,
    lesson::{LessonCompletion, LessonId, LessonSource},
    scheduling::{Booking, BookingId, BookingStatus},
};

/// 同期レッスン（glossary の live_lesson）の通話セッション。1 つの枠の通話につき 1 つ。
///
/// 開始（[`LiveLessonSession::start`]）した直後は [`SessionStatus::InProgress`] で、終了
/// （[`LiveLessonSession::end`]）すると接続品質の生の指標（[`ConnectionQuality`]）を持つ
/// [`SessionStatus::Ended`] になる。セッションには、開始したときに確定していた同じ枠の予約がすべて結び付く
/// （[`SessionBooking`]。マンツーマンの枠では 1 つ、グループレッスンの枠では人数分。変更シナリオ cs-004）。
/// 参加者はコーチと、結び付いた予約の生徒たち。開始・終了できるのは参加者だけ。
#[derive(Debug, Clone, PartialEq)]
pub struct LiveLessonSession {
    id: SessionId,
    booking_id: BookingId,
    student_id: AccountId,
    coach_id: AccountId,
    /// 結び付いた予約。先頭は開始した予約（`booking_id` / `student_id`）
    bookings: Vec<SessionBooking>,
    started_at: SystemTime,
    ended_at: Option<SystemTime>,
    quality: Option<ConnectionQuality>,
}

impl LiveLessonSession {
    /// 予約の参加者 `participant` が `booking` の通話を `now` に始める。
    ///
    /// `slot_bookings` は `booking` と同じ枠の予約（状態を問わない。`booking` 自身を含んでもよい）で、そのうち
    /// 確定したものをすべてセッションに結び付ける（グループレッスン）。`existing` はその予約の既存のセッション
    /// （なければ `None`）。次の不変条件を破れば [`DomainError`] を返す。
    ///
    /// - 開始できるのは予約の生徒かコーチだけ（[`DomainError::NotASessionParticipant`]）
    /// - 確定した予約にしか開始できない（[`DomainError::BookingNotConfirmed`]）
    /// - 同じ予約のセッションは 1 つ（[`DomainError::LiveLessonSessionAlreadyStarted`]。既存のグループレッスンの
    ///   セッションに入るのは [`LiveLessonSession::ensure_can_join`]）
    pub fn start(
        id: SessionId,
        participant: AccountId,
        booking: &Booking,
        slot_bookings: &[Booking],
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
        if existing.is_some_and(|session| session.links(booking.id())) {
            return Err(DomainError::LiveLessonSessionAlreadyStarted);
        }
        let others = slot_bookings
            .iter()
            .filter(|other| {
                other.slot_id() == booking.slot_id() && other.status() == BookingStatus::Confirmed
            })
            .map(|other| SessionBooking::new(other.id(), other.student_id()));
        Ok(Self {
            id,
            booking_id: booking.id(),
            student_id: booking.student_id(),
            coach_id: booking.coach_id(),
            bookings: linked(
                SessionBooking::new(booking.id(), booking.student_id()),
                others,
            ),
            started_at: now,
            ended_at: None,
            quality: None,
        })
    }

    /// 保存先から読んだセッションを組み立てる（保存先の実装用）。`bookings` は結び付いた予約（開始した予約
    /// `booking_id` を含んでも含まなくてもよい。含まなければ先頭に足す。並びは開始した予約のあとに保つ）。
    /// `ended` は終了した時刻と、そのときに記録した接続品質（終了するまでは `None`）。終了が開始より前なら
    /// [`DomainError::SessionEndsBeforeItStarts`]。
    pub fn restore(
        id: SessionId,
        booking_id: BookingId,
        student_id: AccountId,
        coach_id: AccountId,
        bookings: Vec<SessionBooking>,
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
            bookings: linked(SessionBooking::new(booking_id, student_id), bookings),
            started_at,
            ended_at: ended.map(|(ended_at, _)| ended_at),
            quality: ended.map(|(_, quality)| quality),
        })
    }

    /// 参加者 `participant` が予約 `booking` で、このセッション（開始済み）の通話に入れるかを確かめる。
    ///
    /// グループレッスン（結び付いた予約が 2 つ以上）の通話中のセッションには、結び付いた予約の生徒かコーチが
    /// どの予約からでも入れる（同じセッションを返す）。判定は [`LiveLessonSession::start`] と同じ順で、
    /// 予約の生徒かコーチでなければ [`DomainError::NotASessionParticipant`]、確定した予約でなければ
    /// [`DomainError::BookingNotConfirmed`]。マンツーマンのセッション・終了したセッション・結び付いていない予約は
    /// 今までどおり [`DomainError::LiveLessonSessionAlreadyStarted`]（同じ予約のセッションは 1 つ）。
    pub fn ensure_can_join(
        &self,
        participant: AccountId,
        booking: &Booking,
    ) -> Result<(), DomainError> {
        if participant != booking.student_id() && participant != booking.coach_id() {
            return Err(DomainError::NotASessionParticipant);
        }
        if booking.status() != BookingStatus::Confirmed {
            return Err(DomainError::BookingNotConfirmed {
                status: booking.status(),
            });
        }
        if !self.is_group() || self.ended_at.is_some() || !self.links(booking.id()) {
            return Err(DomainError::LiveLessonSessionAlreadyStarted);
        }
        Ok(())
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
    /// 誰の完了かは、`participant` が結び付いた予約の生徒なら本人、コーチなら開始した予約の生徒
    /// （マンツーマンのセッションの生徒）。生徒を選ぶときは [`LiveLessonSession::complete_for`]。
    pub fn complete(
        &self,
        lesson_id: LessonId,
        participant: AccountId,
        history: &[LessonCompletion],
        now: SystemTime,
    ) -> Result<LessonCompletion, DomainError> {
        let student = if self.links_student(participant) {
            participant
        } else {
            self.student_id
        };
        self.complete_for(lesson_id, participant, student, history, now)
    }

    /// 参加者 `participant` が `now` に、結び付いた予約の生徒 `student` のこのセッションの同期レッスンの完了を
    /// 記録する（完了記録の ID は `lesson_id`）。完了したレッスンは結び付いた予約（生徒）ごとに 1 つ。
    ///
    /// `history` はその生徒の既存の完了記録（同期・非同期の両方。`is_first_lesson` の判定と重複の検出に使う。
    /// [`LessonCompletion`]）。次の不変条件を破れば [`DomainError`] を返す。セッションは変えない。
    ///
    /// - 記録できるのはコーチか、その生徒本人だけ（[`DomainError::NotASessionParticipant`]。`student` が結び付いた
    ///   予約の生徒でないときも）
    /// - 終了したセッションだけ（[`DomainError::LiveLessonSessionNotEnded`]）
    /// - 記録は終了の後（同じ時刻は許す。[`DomainError::CompletionBeforeLessonEnded`]）
    /// - 同じセッションの同じ生徒の完了は 1 回（[`DomainError::LessonAlreadyCompleted`]）
    pub fn complete_for(
        &self,
        lesson_id: LessonId,
        participant: AccountId,
        student: AccountId,
        history: &[LessonCompletion],
        now: SystemTime,
    ) -> Result<LessonCompletion, DomainError> {
        if !self.links_student(student) || (participant != student && participant != self.coach_id)
        {
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
            student,
            self.coach_id,
            LessonSource::Live(self.id),
            history,
            now,
        )
    }

    /// コーチか、結び付いた予約の生徒か。
    pub fn is_participant(&self, account: AccountId) -> bool {
        account == self.coach_id || self.links_student(account)
    }

    /// グループレッスンのセッション（結び付いた予約が 2 つ以上）か。
    pub fn is_group(&self) -> bool {
        self.bookings.len() > 1
    }

    fn links(&self, booking_id: BookingId) -> bool {
        self.bookings.iter().any(|b| b.booking_id() == booking_id)
    }

    fn links_student(&self, account: AccountId) -> bool {
        self.bookings.iter().any(|b| b.student_id() == account)
    }

    pub const fn id(&self) -> SessionId {
        self.id
    }

    /// 開始した予約。
    pub const fn booking_id(&self) -> BookingId {
        self.booking_id
    }

    /// 開始した予約の生徒。
    pub const fn student_id(&self) -> AccountId {
        self.student_id
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    /// 結び付いた予約（先頭は開始した予約）。
    pub fn bookings(&self) -> &[SessionBooking] {
        &self.bookings
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

/// 開始した予約 `first` を先頭に、`others` を同じ予約・同じ生徒を除いて並べる。
fn linked(
    first: SessionBooking,
    others: impl IntoIterator<Item = SessionBooking>,
) -> Vec<SessionBooking> {
    let mut bookings = vec![first];
    for other in others {
        if !bookings
            .iter()
            .any(|b| b.booking_id() == other.booking_id() || b.student_id() == other.student_id())
        {
            bookings.push(other);
        }
    }
    bookings
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
                LiveLessonSession::start(SESSION, participant, &booking, &[], None, at(600))
                    .unwrap();
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
            LiveLessonSession::start(SESSION, STUDENT, &pending, &[], None, at(600)),
            Err(DomainError::BookingNotConfirmed {
                status: BookingStatus::Pending
            })
        );
        let mut cancelled = pending.clone();
        cancelled.cancel(STUDENT, &mut slot, NOW).unwrap();
        assert_eq!(
            LiveLessonSession::start(SESSION, STUDENT, &cancelled, &[], None, at(600)),
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
                LiveLessonSession::start(SESSION, OTHER_STUDENT, &booking, &[], None, at(600)),
                Err(DomainError::NotASessionParticipant)
            );
        }
    }

    #[test]
    fn a_booking_has_at_most_one_session() {
        let booking = confirmed();
        let first =
            LiveLessonSession::start(SESSION, STUDENT, &booking, &[], None, at(600)).unwrap();
        let mut ended = first.clone();
        ended.end(STUDENT, at(650), quality()).unwrap();
        for existing in [first, ended] {
            assert_eq!(
                LiveLessonSession::start(
                    SessionId::from_u128(51),
                    COACH,
                    &booking,
                    &[],
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
                LiveLessonSession::start(SESSION, STUDENT, &booking, &[], None, at(600)).unwrap();
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
            LiveLessonSession::start(SESSION, STUDENT, &confirmed(), &[], None, at(600)).unwrap();
        session.end(STUDENT, at(600), quality()).unwrap();
        assert_eq!(session.duration(), Some(Duration::ZERO));
    }

    #[test]
    fn end_failures_change_nothing() {
        let started =
            LiveLessonSession::start(SESSION, STUDENT, &confirmed(), &[], None, at(600)).unwrap();
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
            LiveLessonSession::start(SESSION, STUDENT, &confirmed(), &[], None, at(600)).unwrap();
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
            LiveLessonSession::start(SESSION, STUDENT, &confirmed(), &[], None, at(600)).unwrap();
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
            LiveLessonSession::start(SESSION, STUDENT, &confirmed(), &[], None, at(600)).unwrap();
        session.end(COACH, at(650), quality()).unwrap();

        let restored = LiveLessonSession::restore(
            SESSION,
            session.booking_id(),
            STUDENT,
            COACH,
            vec![],
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
                vec![],
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
                vec![],
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

    /// 定員 3 の枠の 3 人の予約（生徒 1・4・5）。`confirmed` 人目までを確定する。
    fn group(confirmed: usize) -> Vec<Booking> {
        let mut slot = Slot::restore(
            SlotId::from_u128(11),
            COACH,
            Sport::Tennis,
            at(600),
            at(660),
            SlotStatus::Open,
            crate::scheduling::SlotSeats::vacant(crate::scheduling::SlotCapacity::new(3).unwrap()),
        )
        .unwrap();
        let mut bookings: Vec<Booking> = Vec::new();
        for (i, student) in GROUP.into_iter().enumerate() {
            let booking = Booking::reserve(
                BookingId::from_u128(30 + i as u128),
                student,
                false,
                &mut slot,
                &bookings,
                NOW,
            )
            .unwrap();
            bookings.push(booking);
        }
        for booking in bookings.iter_mut().take(confirmed) {
            let student = booking.student_id();
            booking.confirm(student, &mut slot, NOW).unwrap();
        }
        bookings
    }

    const GROUP: [AccountId; 3] = [STUDENT, AccountId::from_u128(4), AccountId::from_u128(5)];

    fn linked_students(session: &LiveLessonSession) -> Vec<AccountId> {
        session.bookings().iter().map(|b| b.student_id()).collect()
    }

    #[test]
    fn a_session_links_every_confirmed_booking_of_the_slot() {
        let bookings = group(3);
        let session =
            LiveLessonSession::start(SESSION, COACH, &bookings[1], &bookings, None, at(600))
                .unwrap();
        // 先頭は開始した予約
        assert_eq!(session.booking_id(), bookings[1].id());
        assert_eq!(session.student_id(), GROUP[1]);
        assert_eq!(
            linked_students(&session),
            vec![GROUP[1], GROUP[0], GROUP[2]]
        );
        assert!(session.is_group());
        for participant in [COACH, GROUP[0], GROUP[1], GROUP[2]] {
            assert!(session.is_participant(participant));
        }
        assert!(!session.is_participant(OTHER_STUDENT));

        // 確定していない予約は結び付けない
        let bookings = group(2);
        let session =
            LiveLessonSession::start(SESSION, COACH, &bookings[0], &bookings, None, at(600))
                .unwrap();
        assert_eq!(linked_students(&session), vec![GROUP[0], GROUP[1]]);
        assert!(!session.is_participant(GROUP[2]));

        // マンツーマンの枠は 1 つだけ
        let booking = confirmed();
        let session = LiveLessonSession::start(
            SESSION,
            COACH,
            &booking,
            std::slice::from_ref(&booking),
            None,
            at(600),
        )
        .unwrap();
        assert_eq!(linked_students(&session), vec![STUDENT]);
        assert!(!session.is_group());
    }

    #[test]
    fn other_bookings_of_a_group_session_join_the_same_session() {
        let bookings = group(3);
        let session =
            LiveLessonSession::start(SESSION, COACH, &bookings[0], &bookings, None, at(600))
                .unwrap();
        for (booking, participant) in [
            (&bookings[1], GROUP[1]),
            (&bookings[2], COACH),
            (&bookings[0], GROUP[0]),
        ] {
            session.ensure_can_join(participant, booking).unwrap();
        }
        // 予約の生徒・コーチでなければ入れない
        assert_eq!(
            session.ensure_can_join(GROUP[2], &bookings[1]),
            Err(DomainError::NotASessionParticipant)
        );
        // 終了したら入れない
        let mut ended = session.clone();
        ended.end(COACH, at(650), quality()).unwrap();
        assert_eq!(
            ended.ensure_can_join(GROUP[1], &bookings[1]),
            Err(DomainError::LiveLessonSessionAlreadyStarted)
        );
        // 開始の後に確定した、結び付いていない予約では入れない
        let late = group(3);
        let early =
            LiveLessonSession::start(SESSION, COACH, &late[0], &late[..2], None, at(600)).unwrap();
        assert_eq!(
            early.ensure_can_join(GROUP[2], &late[2]),
            Err(DomainError::LiveLessonSessionAlreadyStarted)
        );
    }

    #[test]
    fn a_one_to_one_session_is_not_joined_again() {
        let booking = confirmed();
        let session =
            LiveLessonSession::start(SESSION, STUDENT, &booking, &[], None, at(600)).unwrap();
        for participant in [STUDENT, COACH] {
            assert_eq!(
                session.ensure_can_join(participant, &booking),
                Err(DomainError::LiveLessonSessionAlreadyStarted)
            );
        }
    }

    #[test]
    fn a_group_session_is_completed_per_linked_booking() {
        let bookings = group(3);
        let mut session =
            LiveLessonSession::start(SESSION, COACH, &bookings[0], &bookings, None, at(600))
                .unwrap();
        session.end(COACH, at(650), quality()).unwrap();

        // コーチは結び付いた生徒の誰の完了も、生徒は自分の完了だけを記録できる
        let mut history = Vec::new();
        for (i, student) in GROUP.into_iter().enumerate() {
            let completion = session
                .complete_for(
                    LessonId::from_u128(80 + i as u128),
                    COACH,
                    student,
                    &history,
                    at(651),
                )
                .unwrap();
            assert_eq!(completion.student_id(), student);
            assert_eq!(completion.source(), LessonSource::Live(SESSION));
            history.push(completion);
        }
        assert_eq!(
            session.complete_for(LESSON, GROUP[1], GROUP[2], &[], at(651)),
            Err(DomainError::NotASessionParticipant)
        );
        assert_eq!(
            session.complete_for(LESSON, COACH, OTHER_STUDENT, &[], at(651)),
            Err(DomainError::NotASessionParticipant)
        );
        // 生徒の complete は本人の完了
        assert_eq!(
            session
                .complete(LESSON, GROUP[2], &[], at(651))
                .unwrap()
                .student_id(),
            GROUP[2]
        );
        assert_eq!(
            session.complete_for(LESSON, GROUP[1], GROUP[1], &history, at(651)),
            Err(DomainError::LessonAlreadyCompleted)
        );
    }

    #[test]
    fn restore_keeps_the_linked_bookings_after_the_starting_one() {
        let others = [
            SessionBooking::new(BookingId::from_u128(31), GROUP[1]),
            SessionBooking::new(BookingId::from_u128(20), STUDENT),
        ];
        let session = LiveLessonSession::restore(
            SESSION,
            BookingId::from_u128(20),
            STUDENT,
            COACH,
            others.to_vec(),
            at(600),
            None,
        )
        .unwrap();
        assert_eq!(linked_students(&session), vec![STUDENT, GROUP[1]]);
    }
}
