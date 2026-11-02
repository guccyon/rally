use std::{
    cmp::Reverse,
    time::{Duration, SystemTime},
};

use super::LiveLessonSession;
use crate::{DomainError, identity::AccountId};

/// 本部が見る通話セッションの記録の一覧の条件（admin-web の `listLiveLessonSessions`。期間・範囲・並べ方の正本は
/// `specs/contracts/api/admin-web/openapi.yaml` の `listLiveLessonSessions`）。
///
/// 開始 [`Self::starts_at`]（含む）から終了 [`Self::ends_at`]（含まない）までに開始したセッションを、任意のコーチ
/// [`Self::coach_id`] と任意の生徒 [`Self::student_id`] で絞る。生徒は結び付いた予約の生徒（グループレッスンでは開始した
/// 予約の生徒に限らない。[`LiveLessonSession::booking_of`]）で見る。日付から時刻への変換（タイムゾーン）は application が決め、ここは期間の長さの規則と
/// 選び方・並べ方だけを持つ。期間は 0 より長く [`Self::MAX_DAYS`] 日まで。終了の有無は問わない（通話中も含む）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionHistoryCriteria {
    starts_at: SystemTime,
    ends_at: SystemTime,
    coach_id: Option<AccountId>,
    student_id: Option<AccountId>,
}

impl SessionHistoryCriteria {
    /// 1 日の長さ（日本時間に夏時間はないので、どの日も 24 時間）
    pub const DAY: Duration = Duration::from_secs(24 * 3600);
    /// 期間の日数の上限（始まりの日と終わりの日を含めて 31 日）
    pub const MAX_DAYS: u16 = 31;

    /// 終了が開始より後でなければ [`DomainError::SessionPeriodEndsBeforeItStarts`]、[`Self::MAX_DAYS`] 日より
    /// 長ければ [`DomainError::SessionPeriodTooLong`]。`coach_id` があればそのコーチの、`student_id` があればその生徒の
    /// セッションだけを選ぶ。
    pub fn new(
        starts_at: SystemTime,
        ends_at: SystemTime,
        coach_id: Option<AccountId>,
        student_id: Option<AccountId>,
    ) -> Result<Self, DomainError> {
        let length = match ends_at.duration_since(starts_at) {
            Ok(length) if !length.is_zero() => length,
            _ => return Err(DomainError::SessionPeriodEndsBeforeItStarts),
        };
        if length > Self::DAY * u32::from(Self::MAX_DAYS) {
            return Err(DomainError::SessionPeriodTooLong {
                max_days: Self::MAX_DAYS,
            });
        }
        Ok(Self {
            starts_at,
            ends_at,
            coach_id,
            student_id,
        })
    }

    pub const fn starts_at(&self) -> SystemTime {
        self.starts_at
    }

    pub const fn ends_at(&self) -> SystemTime {
        self.ends_at
    }

    pub const fn coach_id(&self) -> Option<AccountId> {
        self.coach_id
    }

    pub const fn student_id(&self) -> Option<AccountId> {
        self.student_id
    }

    /// `session` が期間に開始し（開始を含み、終了を含まない）、コーチの指定があればそのコーチの、生徒の指定があれば
    /// その生徒の予約が結び付いたセッションか。
    pub fn matches(&self, session: &LiveLessonSession) -> bool {
        self.starts_at <= session.started_at()
            && session.started_at() < self.ends_at
            && self
                .coach_id
                .is_none_or(|coach_id| session.coach_id() == coach_id)
            && self
                .student_id
                .is_none_or(|student_id| session.booking_of(student_id).is_some())
    }

    /// `sessions` から条件に合うものだけを選び、開始の新しい順（同じ時刻はセッションの ID の順）に並べる。
    pub fn select(&self, sessions: Vec<LiveLessonSession>) -> Vec<LiveLessonSession> {
        let mut selected: Vec<LiveLessonSession> = sessions
            .into_iter()
            .filter(|session| self.matches(session))
            .collect();
        selected.sort_by_key(|session| (Reverse(session.started_at()), session.id()));
        selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        live_lesson::{ConnectionQuality, SessionBooking, SessionId},
        scheduling::BookingId,
    };

    const HOUR: Duration = Duration::from_secs(3600);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);

    fn day(n: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + SessionHistoryCriteria::DAY * n as u32
    }

    fn session(
        id: u128,
        coach: AccountId,
        started_at: SystemTime,
        ended_at: Option<SystemTime>,
    ) -> LiveLessonSession {
        LiveLessonSession::restore(
            SessionId::from_u128(id),
            BookingId::from_u128(100 + id),
            STUDENT,
            coach,
            vec![],
            started_at,
            ended_at.map(|ended_at| (ended_at, ConnectionQuality::new(0.0, 0, 0, 0).unwrap())),
        )
        .unwrap()
    }

    fn ids(sessions: &[LiveLessonSession]) -> Vec<u128> {
        sessions.iter().map(|s| s.id().as_u128()).collect()
    }

    #[test]
    fn accepts_periods_up_to_max_days() {
        let one = SessionHistoryCriteria::new(day(10), day(11), None, None).unwrap();
        assert_eq!((one.starts_at(), one.ends_at()), (day(10), day(11)));
        assert_eq!((one.coach_id(), one.student_id()), (None, None));
        let longest = SessionHistoryCriteria::new(
            day(10),
            day(10 + u64::from(SessionHistoryCriteria::MAX_DAYS)),
            Some(COACH),
            Some(STUDENT),
        )
        .unwrap();
        assert_eq!(longest.coach_id(), Some(COACH));
        assert_eq!(longest.student_id(), Some(STUDENT));
    }

    #[test]
    fn rejects_periods_that_end_before_or_when_they_start() {
        for ends_at in [day(10), day(9)] {
            assert_eq!(
                SessionHistoryCriteria::new(day(10), ends_at, None, None),
                Err(DomainError::SessionPeriodEndsBeforeItStarts)
            );
        }
    }

    #[test]
    fn rejects_periods_longer_than_max_days() {
        let ends_at = day(10 + u64::from(SessionHistoryCriteria::MAX_DAYS));
        assert_eq!(
            SessionHistoryCriteria::new(day(10), ends_at + Duration::from_secs(1), None, None),
            Err(DomainError::SessionPeriodTooLong { max_days: 31 })
        );
    }

    #[test]
    fn matches_sessions_started_in_the_period_whether_ended_or_not() {
        let criteria = SessionHistoryCriteria::new(day(10), day(12), None, None).unwrap();
        assert!(criteria.matches(&session(1, COACH, day(10), None)));
        assert!(criteria.matches(&session(1, COACH, day(10), Some(day(10) + HOUR))));
        assert!(criteria.matches(&session(1, COACH, day(12) - Duration::from_nanos(1), None)));
        // 終了が期間の後でも、期間に開始していれば含む
        assert!(criteria.matches(&session(1, COACH, day(11), Some(day(13)))));
        assert!(!criteria.matches(&session(1, COACH, day(12), None)));
        assert!(!criteria.matches(&session(
            1,
            COACH,
            day(10) - Duration::from_nanos(1),
            Some(day(10) + HOUR)
        )));
    }

    #[test]
    fn the_coach_narrows_the_sessions() {
        let criteria = SessionHistoryCriteria::new(day(10), day(12), Some(COACH), None).unwrap();
        assert!(criteria.matches(&session(1, COACH, day(10), None)));
        assert!(!criteria.matches(&session(1, OTHER_COACH, day(10), None)));
    }

    #[test]
    fn the_student_narrows_the_sessions() {
        let criteria =
            SessionHistoryCriteria::new(day(10), day(12), None, Some(OTHER_STUDENT)).unwrap();
        assert!(!criteria.matches(&session(1, COACH, day(10), None)));
        // OTHER_STUDENT が開始した予約の生徒
        let started_by_other = LiveLessonSession::restore(
            SessionId::from_u128(2),
            BookingId::from_u128(102),
            OTHER_STUDENT,
            COACH,
            vec![],
            day(10),
            None,
        )
        .unwrap();
        assert!(criteria.matches(&started_by_other));
        // グループレッスンで、STUDENT が開始したセッションに OTHER_STUDENT の予約も結び付いている
        let group = LiveLessonSession::restore(
            SessionId::from_u128(3),
            BookingId::from_u128(103),
            STUDENT,
            COACH,
            vec![SessionBooking::new(
                BookingId::from_u128(104),
                OTHER_STUDENT,
            )],
            day(10),
            None,
        )
        .unwrap();
        assert!(criteria.matches(&group), "結び付いた予約の生徒も含む");
        // 生徒の指定は期間・コーチの指定と組み合わさる
        assert!(
            !SessionHistoryCriteria::new(day(10), day(12), Some(OTHER_COACH), Some(OTHER_STUDENT))
                .unwrap()
                .matches(&group)
        );
        assert!(
            !SessionHistoryCriteria::new(day(11), day(12), None, Some(OTHER_STUDENT))
                .unwrap()
                .matches(&group)
        );
    }

    #[test]
    fn selects_sessions_newest_first() {
        let sessions = vec![
            session(1, COACH, day(10) + HOUR, Some(day(10) + 2 * HOUR)),
            session(2, COACH, day(9), Some(day(9) + HOUR)),
            session(3, OTHER_COACH, day(11), None),
            session(5, COACH, day(10) + 5 * HOUR, None),
            session(4, COACH, day(10) + 5 * HOUR, Some(day(10) + 6 * HOUR)),
            session(6, COACH, day(12), None),
        ];
        assert_eq!(
            ids(&SessionHistoryCriteria::new(day(10), day(12), None, None)
                .unwrap()
                .select(sessions.clone())),
            [3, 4, 5, 1],
            "同じ開始時刻はセッションの ID の順"
        );
        assert_eq!(
            ids(
                &SessionHistoryCriteria::new(day(10), day(12), Some(COACH), None)
                    .unwrap()
                    .select(sessions)
            ),
            [4, 5, 1]
        );
    }
}
