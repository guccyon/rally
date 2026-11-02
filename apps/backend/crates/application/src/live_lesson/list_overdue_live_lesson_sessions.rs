use std::collections::{BTreeMap, BTreeSet};

use domain::{identity::AccountId, live_lesson::OverdueSessionCriteria};

use super::{
    LiveLessonSessionRepository, OverdueSessionList, SessionCoachDisplayNameLookup,
    SessionStudentDisplayNameLookup,
};
use crate::{Unavailable, clock::Clock};

/// 本部が見る、終わっていないまま時間の過ぎた通話セッションの一覧のユースケース（admin-web の
/// `listOverdueLiveLessonSessions`。本部だけが呼ぶ）。照会だけで、何も保存しない（セッションを終了させない）。
///
/// 基準（domain の [`OverdueSessionCriteria`]。開始からの時間）の時刻
/// （[`OverdueSessionCriteria::started_at_or_before`]）までに開始した通話中のセッションを読み、
/// [`OverdueSessionCriteria::select`] で選んで並べる（既定・範囲・境界は domain だけ）。各セッションには、コーチと
/// 開始した予約の生徒の表示名（[`SessionCoachDisplayNameLookup`]・[`SessionStudentDisplayNameLookup`]）を引いて添える。
/// 誰が呼べるか（本部だけ）は境界（interface）が確かめる。
pub struct ListOverdueLiveLessonSessions<L, D, S, C> {
    sessions: L,
    coach_names: D,
    student_names: S,
    clock: C,
}

impl<L, D, S, C> ListOverdueLiveLessonSessions<L, D, S, C>
where
    L: LiveLessonSessionRepository,
    D: SessionCoachDisplayNameLookup,
    S: SessionStudentDisplayNameLookup,
    C: Clock,
{
    pub fn new(sessions: L, coach_names: D, student_names: S, clock: C) -> Self {
        Self {
            sessions,
            coach_names,
            student_names,
            clock,
        }
    }

    pub async fn execute(
        &self,
        criteria: OverdueSessionCriteria,
    ) -> Result<OverdueSessionList, Unavailable> {
        let now = self.clock.now();
        let in_progress = self
            .sessions
            .find_in_progress_started_at_or_before(criteria.started_at_or_before(now))
            .await?;
        let sessions = criteria.select(in_progress, now);
        if sessions.is_empty() {
            return Ok(OverdueSessionList {
                criteria,
                listed_at: now,
                sessions,
                coach_display_names: BTreeMap::new(),
                student_display_names: BTreeMap::new(),
            });
        }

        let coach_ids: BTreeSet<AccountId> = sessions.iter().map(|s| s.coach_id()).collect();
        let student_ids: BTreeSet<AccountId> = sessions.iter().map(|s| s.student_id()).collect();
        let coach_display_names = self
            .coach_names
            .find_coach_display_names(&coach_ids)
            .await?;
        let student_display_names = self
            .student_names
            .find_student_display_names(&student_ids)
            .await?;
        Ok(OverdueSessionList {
            criteria,
            listed_at: now,
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

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryLiveLessonSessionRepository};

    const HOUR: Duration = Duration::from_secs(3600);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn now() -> SystemTime {
        // 2026-11-01T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200)
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

    fn use_case(
        sessions: impl IntoIterator<Item = LiveLessonSession>,
        lookup: RecordingLookup,
    ) -> ListOverdueLiveLessonSessions<
        InMemoryLiveLessonSessionRepository,
        RecordingLookup,
        RecordingLookup,
        FixedClock,
    > {
        ListOverdueLiveLessonSessions::new(
            InMemoryLiveLessonSessionRepository::with_sessions(sessions),
            lookup.clone(),
            lookup,
            FixedClock(now()),
        )
    }

    fn ids(list: &OverdueSessionList) -> Vec<u128> {
        list.sessions.iter().map(|s| s.id().as_u128()).collect()
    }

    #[tokio::test]
    async fn lists_in_progress_sessions_started_three_hours_ago_or_earlier_oldest_first() {
        let lookup = RecordingLookup::default();
        let list = use_case(
            [
                session(1, STUDENT, COACH, now() - 3 * HOUR, None),
                // 3 時間に 1 秒足りない
                session(
                    2,
                    STUDENT,
                    COACH,
                    now() - 3 * HOUR + Duration::from_secs(1),
                    None,
                ),
                // 終了したセッションは出ない
                session(3, STUDENT, COACH, now() - 10 * HOUR, Some(now() - 9 * HOUR)),
                session(4, OTHER_STUDENT, OTHER_COACH, now() - 26 * HOUR, None),
                session(5, STUDENT, OTHER_COACH, now() - 5 * HOUR, None),
            ],
            lookup.clone(),
        )
        .execute(OverdueSessionCriteria::default())
        .await
        .unwrap();
        assert_eq!(list.criteria.hours(), 3);
        assert_eq!(list.listed_at, now());
        assert_eq!(ids(&list), [4, 5, 1]);
        assert_eq!(list.sessions[0].elapsed(list.listed_at), 26 * HOUR);
        assert_eq!(
            list.coach_display_names,
            BTreeMap::from([(COACH, name("たなか"))])
        );
        assert_eq!(
            list.student_display_names,
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
    async fn the_hours_of_the_criteria_change_the_threshold() {
        let sessions = [
            session(1, STUDENT, COACH, now() - HOUR, None),
            session(2, STUDENT, COACH, now() - 5 * HOUR, None),
            session(3, STUDENT, COACH, now() - 30 * HOUR, None),
        ];
        for (hours, expected) in [(1, vec![3, 2, 1]), (6, vec![3]), (168, vec![])] {
            let list = use_case(sessions.clone(), RecordingLookup::default())
                .execute(OverdueSessionCriteria::new(hours).unwrap())
                .await
                .unwrap();
            assert_eq!(list.criteria.hours(), hours);
            assert_eq!(ids(&list), expected, "{hours} 時間");
        }
    }

    #[tokio::test]
    async fn no_overdue_sessions_is_an_empty_list_without_asking_for_display_names() {
        let lookup = RecordingLookup::default();
        let list = use_case(
            [
                session(1, STUDENT, COACH, now() - HOUR, None),
                session(2, STUDENT, COACH, now() - 10 * HOUR, Some(now() - 9 * HOUR)),
            ],
            lookup.clone(),
        )
        .execute(OverdueSessionCriteria::default())
        .await
        .unwrap();
        assert!(list.sessions.is_empty());
        assert!(list.coach_display_names.is_empty() && list.student_display_names.is_empty());
        assert!(lookup.coaches_asked.lock().unwrap().is_empty());
        assert!(lookup.students_asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn listing_does_not_change_the_sessions() {
        let overdue = session(1, STUDENT, COACH, now() - 5 * HOUR, None);
        let repository = InMemoryLiveLessonSessionRepository::with_sessions([overdue.clone()]);
        ListOverdueLiveLessonSessions::new(
            repository.clone(),
            RecordingLookup::default(),
            RecordingLookup::default(),
            FixedClock(now()),
        )
        .execute(OverdueSessionCriteria::default())
        .await
        .unwrap();
        assert_eq!(repository.all(), [overdue], "セッションを終了させない");
    }
}
