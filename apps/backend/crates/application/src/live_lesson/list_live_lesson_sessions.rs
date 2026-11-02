use std::collections::{BTreeMap, BTreeSet};

use domain::{DomainError, identity::AccountId, live_lesson::SessionHistoryCriteria};
use time::Date;

use super::{
    LiveLessonSessionHistory, LiveLessonSessionRepository, SessionCoachDisplayNameLookup,
    SessionStudentDisplayNameLookup,
};
use crate::{Unavailable, scheduling::calendar_day};

/// 本部が期間とコーチで見る通話セッションの記録の一覧のユースケース（admin-web の `listLiveLessonSessions`。本部だけが
/// 呼ぶ）。問い合わせを受けたときに、ライブレッスンがいつ始まりいつ終わったかを引くための照会で、何も保存しない。
///
/// 日付（日本時間）の `from` から `to` まで（両端を含む）に開始したセッションを、終了の有無を問わず読む。期間の規則
/// （`to` が `from` より前でない・[`SessionHistoryCriteria::MAX_DAYS`] 日まで）と選び方・並べ方（開始の新しい順）は
/// domain の [`SessionHistoryCriteria`]。日付の区切りは予約の一覧と同じ [`calendar_day`]。`coach_id` があればその
/// コーチのセッションだけ（いないコーチなら空の一覧）。各セッションには、コーチと開始した予約の生徒の表示名
/// （[`SessionCoachDisplayNameLookup`]・[`SessionStudentDisplayNameLookup`]）を引いて添える。誰が呼べるか（本部だけ）は
/// 境界（interface）が確かめる。
pub struct ListLiveLessonSessions<L, D, S> {
    sessions: L,
    coach_names: D,
    student_names: S,
}

/// 本部が見る通話セッションの記録の一覧の条件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListLiveLessonSessionsQuery {
    /// 期間の始まりの日付（日本時間。含む）
    pub from: Date,
    /// 期間の終わりの日付（日本時間。含む）
    pub to: Date,
    /// あればこのコーチのセッションだけ
    pub coach_id: Option<AccountId>,
}

#[derive(Debug)]
pub enum ListLiveLessonSessionsError {
    /// 期間が規則を満たさない（[`SessionHistoryCriteria::new`] の違反）
    Invalid(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<DomainError> for ListLiveLessonSessionsError {
    fn from(error: DomainError) -> Self {
        Self::Invalid(error)
    }
}

impl From<Unavailable> for ListLiveLessonSessionsError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}

impl<L, D, S> ListLiveLessonSessions<L, D, S>
where
    L: LiveLessonSessionRepository,
    D: SessionCoachDisplayNameLookup,
    S: SessionStudentDisplayNameLookup,
{
    pub fn new(sessions: L, coach_names: D, student_names: S) -> Self {
        Self {
            sessions,
            coach_names,
            student_names,
        }
    }

    pub async fn execute(
        &self,
        query: ListLiveLessonSessionsQuery,
    ) -> Result<LiveLessonSessionHistory, ListLiveLessonSessionsError> {
        let criteria = SessionHistoryCriteria::new(
            calendar_day::start(query.from),
            calendar_day::end(query.to),
            query.coach_id,
        )?;
        let found = self
            .sessions
            .find_started_between(
                criteria.starts_at(),
                criteria.ends_at(),
                criteria.coach_id(),
            )
            .await?;
        let sessions = criteria.select(found);

        let (coach_display_names, student_display_names) = if sessions.is_empty() {
            (BTreeMap::new(), BTreeMap::new())
        } else {
            let coach_ids: BTreeSet<AccountId> = sessions.iter().map(|s| s.coach_id()).collect();
            let student_ids: BTreeSet<AccountId> =
                sessions.iter().map(|s| s.student_id()).collect();
            (
                self.coach_names
                    .find_coach_display_names(&coach_ids)
                    .await?,
                self.student_names
                    .find_student_display_names(&student_ids)
                    .await?,
            )
        };
        Ok(LiveLessonSessionHistory {
            from: query.from,
            to: query.to,
            sessions,
            coach_display_names,
            student_display_names,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::{Duration, SystemTime},
    };

    use domain::{
        live_lesson::{ConnectionQuality, LiveLessonSession, SessionId},
        profile::DisplayName,
        scheduling::BookingId,
    };
    use time::Month;

    use super::*;
    use crate::in_memory::InMemoryLiveLessonSessionRepository;

    const HOUR: Duration = Duration::from_secs(3600);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn date(month: Month, day: u8) -> Date {
        Date::from_calendar_date(2026, month, day).unwrap()
    }

    /// 2026 年 11 月 `day` 日の日本時間 `hour` 時
    fn at(day: u8, hour: u32) -> SystemTime {
        calendar_day::start(date(Month::November, day)) + hour * HOUR
    }

    fn session(
        id: u128,
        student: AccountId,
        coach: AccountId,
        started_at: SystemTime,
        ended_at: Option<SystemTime>,
    ) -> LiveLessonSession {
        LiveLessonSession::restore(
            SessionId::from_u128(id),
            BookingId::from_u128(100 + id),
            student,
            coach,
            vec![],
            started_at,
            ended_at.map(|ended_at| (ended_at, ConnectionQuality::new(0.0, 0, 0, 0).unwrap())),
        )
        .unwrap()
    }

    fn name(value: &str) -> DisplayName {
        DisplayName::new(value).unwrap()
    }

    /// `COACH` と `STUDENT` の表示名だけを知っていて、問い合わせた ID を記録する。
    #[derive(Clone, Default)]
    struct RecordingLookup {
        coaches_asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
        students_asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
    }

    impl SessionCoachDisplayNameLookup for RecordingLookup {
        async fn find_coach_display_names(
            &self,
            coach_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            self.coaches_asked.lock().unwrap().push(coach_ids.clone());
            Ok(coach_ids
                .iter()
                .filter(|id| **id == COACH)
                .map(|id| (*id, name("たなか")))
                .collect())
        }
    }

    impl SessionStudentDisplayNameLookup for RecordingLookup {
        async fn find_student_display_names(
            &self,
            student_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            self.students_asked
                .lock()
                .unwrap()
                .push(student_ids.clone());
            Ok(student_ids
                .iter()
                .filter(|id| **id == STUDENT)
                .map(|id| (*id, name("あおき")))
                .collect())
        }
    }

    /// 11 月 1 日〜3 日（日本時間）の通話と、その前後の通話。
    fn sessions() -> Vec<LiveLessonSession> {
        vec![
            session(1, STUDENT, COACH, at(1, 0), Some(at(1, 1))),
            // 10 月 31 日の日本時間 23 時（期間の外）
            session(2, STUDENT, COACH, at(1, 0) - HOUR, Some(at(1, 0))),
            session(3, OTHER_STUDENT, OTHER_COACH, at(2, 10), Some(at(2, 11))),
            // 通話中
            session(4, STUDENT, COACH, at(3, 23), None),
            // 11 月 4 日の日本時間 0 時（期間の外）
            session(5, STUDENT, COACH, at(4, 0), None),
        ]
    }

    fn use_case(
        lookup: RecordingLookup,
    ) -> ListLiveLessonSessions<InMemoryLiveLessonSessionRepository, RecordingLookup, RecordingLookup>
    {
        ListLiveLessonSessions::new(
            InMemoryLiveLessonSessionRepository::with_sessions(sessions()),
            lookup.clone(),
            lookup,
        )
    }

    fn query(from: u8, to: u8, coach_id: Option<AccountId>) -> ListLiveLessonSessionsQuery {
        ListLiveLessonSessionsQuery {
            from: date(Month::November, from),
            to: date(Month::November, to),
            coach_id,
        }
    }

    fn ids(history: &LiveLessonSessionHistory) -> Vec<u128> {
        history.sessions.iter().map(|s| s.id().as_u128()).collect()
    }

    #[tokio::test]
    async fn lists_sessions_started_in_the_days_in_japan_newest_first() {
        let lookup = RecordingLookup::default();
        let history = use_case(lookup.clone())
            .execute(query(1, 3, None))
            .await
            .unwrap();
        assert_eq!(
            (history.from, history.to),
            (date(Month::November, 1), date(Month::November, 3))
        );
        assert_eq!(ids(&history), [4, 3, 1], "通話中も含む。期間の外は出ない");
        assert_eq!(
            history.coach_display_names,
            BTreeMap::from([(COACH, name("たなか"))])
        );
        assert_eq!(
            history.student_display_names,
            BTreeMap::from([(STUDENT, name("あおき"))])
        );
        assert_eq!(
            *lookup.coaches_asked.lock().unwrap(),
            [BTreeSet::from([COACH, OTHER_COACH])],
            "一覧のコーチを重ねずに 1 回で問い合わせる"
        );
        assert_eq!(
            *lookup.students_asked.lock().unwrap(),
            [BTreeSet::from([STUDENT, OTHER_STUDENT])]
        );
    }

    #[tokio::test]
    async fn the_coach_narrows_the_sessions() {
        let history = use_case(RecordingLookup::default())
            .execute(query(1, 3, Some(OTHER_COACH)))
            .await
            .unwrap();
        assert_eq!(ids(&history), [3]);
    }

    #[tokio::test]
    async fn a_single_day_is_from_midnight_to_midnight_in_japan() {
        let history = use_case(RecordingLookup::default())
            .execute(ListLiveLessonSessionsQuery {
                from: date(Month::October, 31),
                to: date(Month::October, 31),
                coach_id: None,
            })
            .await
            .unwrap();
        assert_eq!(ids(&history), [2]);
    }

    #[tokio::test]
    async fn no_sessions_is_an_empty_list_without_asking_for_display_names() {
        let lookup = RecordingLookup::default();
        let history = use_case(lookup.clone())
            .execute(query(10, 12, None))
            .await
            .unwrap();
        assert!(history.sessions.is_empty());
        assert!(history.coach_display_names.is_empty() && history.student_display_names.is_empty());
        assert!(lookup.coaches_asked.lock().unwrap().is_empty());
        assert!(lookup.students_asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn periods_that_break_the_rules_are_invalid() {
        for (query, expected) in [
            (
                query(3, 2, None),
                DomainError::SessionPeriodEndsBeforeItStarts,
            ),
            (
                ListLiveLessonSessionsQuery {
                    from: date(Month::November, 1),
                    to: date(Month::December, 2),
                    coach_id: None,
                },
                DomainError::SessionPeriodTooLong { max_days: 31 },
            ),
        ] {
            match use_case(RecordingLookup::default()).execute(query).await {
                Err(ListLiveLessonSessionsError::Invalid(error)) => assert_eq!(error, expected),
                other => panic!("{other:?}"),
            }
        }
        // 11 月 1 日〜12 月 1 日（31 日）は通る
        assert!(
            use_case(RecordingLookup::default())
                .execute(ListLiveLessonSessionsQuery {
                    from: date(Month::November, 1),
                    to: date(Month::December, 1),
                    coach_id: None,
                })
                .await
                .is_ok()
        );
    }
}
