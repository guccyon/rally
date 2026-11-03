use std::collections::{BTreeMap, BTreeSet};

use domain::{identity::AccountId, live_lesson::OverdueSessionCriteria};

use super::{
    LiveLessonSessionRepository, OverdueSessionList, SessionCoachAccountLookup,
    SessionCoachDisplayNameLookup, SessionStudentDisplayNameLookup,
};
use crate::{Unavailable, clock::Clock};

/// 本部が見る、終わっていないまま時間の過ぎた通話セッションの一覧のユースケース（admin-web の
/// `listOverdueLiveLessonSessions`。本部だけが呼ぶ）。照会だけで、何も保存しない（セッションを終了させない）。
///
/// 基準（domain の [`OverdueSessionCriteria`]。開始からの時間）の時刻
/// （[`OverdueSessionCriteria::started_at_or_before`]）までに開始した通話中のセッションを読み、
/// [`OverdueSessionCriteria::select`] で選んで並べる（既定・範囲・境界は domain だけ）。基準にコーチ
/// （[`OverdueSessionCriteria::coach_id`]）があればそのコーチのセッションだけ（いないコーチなら空の一覧）。基準に競技
/// （[`OverdueSessionCriteria::sport`]）があれば、選んだセッションのコーチのアカウント（[`SessionCoachAccountLookup`]）を引き、
/// [`OverdueSessionCriteria::matches_coach_sport`] に合うセッションだけを残す（並べ方は変わらない）。各セッションには、コーチと
/// 開始した予約の生徒の表示名（[`SessionCoachDisplayNameLookup`]・[`SessionStudentDisplayNameLookup`]）を引いて添える。
/// 誰が呼べるか（本部だけ）は境界（interface）が確かめる。
pub struct ListOverdueLiveLessonSessions<L, A, D, S, C> {
    sessions: L,
    coach_accounts: A,
    coach_names: D,
    student_names: S,
    clock: C,
}

impl<L, A, D, S, C> ListOverdueLiveLessonSessions<L, A, D, S, C>
where
    L: LiveLessonSessionRepository,
    A: SessionCoachAccountLookup,
    D: SessionCoachDisplayNameLookup,
    S: SessionStudentDisplayNameLookup,
    C: Clock,
{
    pub fn new(sessions: L, coach_accounts: A, coach_names: D, student_names: S, clock: C) -> Self {
        Self {
            sessions,
            coach_accounts,
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
            .find_in_progress_started_at_or_before(
                criteria.started_at_or_before(now),
                criteria.coach_id(),
            )
            .await?;
        let mut sessions = criteria.select(in_progress, now);
        if criteria.sport().is_some() && !sessions.is_empty() {
            let coach_ids: BTreeSet<AccountId> = sessions.iter().map(|s| s.coach_id()).collect();
            let coaches = self.coach_accounts.find_coach_accounts(&coach_ids).await?;
            sessions
                .retain(|session| criteria.matches_coach_sport(coaches.get(&session.coach_id())));
        }
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
        catalog::Sport,
        identity::{Account, Email, Role},
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

    /// `COACH` と `STUDENT` の表示名だけを知っていて、問い合わせた ID を記録する。コーチのアカウントは `COACH`（テニスと
    /// ゴルフ）と `OTHER_COACH`（サッカー）だけを知っている。
    #[derive(Clone, Default)]
    struct RecordingLookup {
        coaches_asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
        students_asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
        coach_accounts_asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
    }

    fn coach_account(id: AccountId, sports: &[Sport]) -> Account {
        Account::new(
            id,
            Email::new(format!("coach{}@example.com", id.as_u128())).unwrap(),
            name("コーチ"),
            Role::Coach,
            sports.iter().copied(),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    impl SessionCoachAccountLookup for RecordingLookup {
        async fn find_coach_accounts(
            &self,
            coach_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, Account>, Unavailable> {
            self.coach_accounts_asked
                .lock()
                .unwrap()
                .push(coach_ids.clone());
            Ok(coach_ids
                .iter()
                .filter_map(|id| match *id {
                    COACH => Some((*id, coach_account(*id, &[Sport::Tennis, Sport::Golf]))),
                    OTHER_COACH => Some((*id, coach_account(*id, &[Sport::Soccer]))),
                    _ => None,
                })
                .collect())
        }
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
        RecordingLookup,
        FixedClock,
    > {
        ListOverdueLiveLessonSessions::new(
            InMemoryLiveLessonSessionRepository::with_sessions(sessions),
            lookup.clone(),
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
        assert!(
            lookup.coach_accounts_asked.lock().unwrap().is_empty(),
            "競技を指定しなければコーチのアカウントを問い合わせない"
        );
    }

    #[tokio::test]
    async fn the_sport_of_the_criteria_keeps_sessions_of_coaches_offering_it() {
        const UNKNOWN_COACH: AccountId = AccountId::from_u128(99);
        let sessions = [
            session(1, STUDENT, COACH, now() - 4 * HOUR, None),
            session(2, OTHER_STUDENT, OTHER_COACH, now() - 26 * HOUR, None),
            session(3, STUDENT, COACH, now() - 30 * HOUR, None),
            session(4, STUDENT, COACH, now() - HOUR, None),
            session(5, STUDENT, COACH, now() - 10 * HOUR, Some(now() - 9 * HOUR)),
            // アカウントの引けないコーチ
            session(6, STUDENT, UNKNOWN_COACH, now() - 20 * HOUR, None),
        ];
        let lookup = RecordingLookup::default();
        let list = use_case(sessions.clone(), lookup.clone())
            .execute(OverdueSessionCriteria::default().with_sport(Some(Sport::Golf)))
            .await
            .unwrap();
        assert_eq!(list.criteria.sport(), Some(Sport::Golf));
        assert_eq!(
            ids(&list),
            [3, 1],
            "ほかの競技のコーチ・アカウントの引けないコーチ・時間に足りない・終了したものは出ない。開始の古い順のまま"
        );
        assert_eq!(
            *lookup.coach_accounts_asked.lock().unwrap(),
            [BTreeSet::from([COACH, OTHER_COACH, UNKNOWN_COACH])],
            "時間で選んだ後のセッションのコーチを重ねずに 1 回で問い合わせる"
        );
        assert_eq!(
            *lookup.coaches_asked.lock().unwrap(),
            [BTreeSet::from([COACH])],
            "競技で絞った後のセッションのコーチだけ表示名を問い合わせる"
        );

        // 時間・コーチと組み合わさる
        for (criteria, expected) in [
            (
                OverdueSessionCriteria::new(5)
                    .unwrap()
                    .with_sport(Some(Sport::Tennis)),
                vec![3],
            ),
            (
                OverdueSessionCriteria::default().with_sport(Some(Sport::Soccer)),
                vec![2],
            ),
            (
                OverdueSessionCriteria::default()
                    .with_coach_id(Some(OTHER_COACH))
                    .with_sport(Some(Sport::Soccer)),
                vec![2],
            ),
            (
                OverdueSessionCriteria::default()
                    .with_coach_id(Some(COACH))
                    .with_sport(Some(Sport::Soccer)),
                vec![],
            ),
            (
                OverdueSessionCriteria::default().with_sport(Some(Sport::Swimming)),
                vec![],
            ),
        ] {
            let list = use_case(sessions.clone(), RecordingLookup::default())
                .execute(criteria)
                .await
                .unwrap();
            assert_eq!(ids(&list), expected, "{criteria:?}");
        }

        // 競技で 1 件も残らなければ、表示名を問い合わせない
        let lookup = RecordingLookup::default();
        let list = use_case(sessions, lookup.clone())
            .execute(OverdueSessionCriteria::default().with_sport(Some(Sport::Baseball)))
            .await
            .unwrap();
        assert!(list.sessions.is_empty());
        assert!(lookup.coaches_asked.lock().unwrap().is_empty());
        assert!(lookup.students_asked.lock().unwrap().is_empty());
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
    async fn the_coach_of_the_criteria_narrows_the_sessions_with_the_hours() {
        let sessions = [
            session(1, STUDENT, COACH, now() - 4 * HOUR, None),
            session(2, OTHER_STUDENT, OTHER_COACH, now() - 26 * HOUR, None),
            session(3, STUDENT, COACH, now() - 30 * HOUR, None),
            session(4, STUDENT, COACH, now() - HOUR, None),
            session(5, STUDENT, COACH, now() - 10 * HOUR, Some(now() - 9 * HOUR)),
        ];
        let lookup = RecordingLookup::default();
        let list = use_case(sessions.clone(), lookup.clone())
            .execute(OverdueSessionCriteria::default().with_coach_id(Some(COACH)))
            .await
            .unwrap();
        assert_eq!(list.criteria.coach_id(), Some(COACH));
        assert_eq!(
            ids(&list),
            [3, 1],
            "ほかのコーチ・時間に足りない・終了したものは出ない"
        );
        assert_eq!(
            *lookup.coaches_asked.lock().unwrap(),
            [BTreeSet::from([COACH])],
            "絞った後のセッションのコーチだけを問い合わせる"
        );
        assert_eq!(
            list.student_display_names,
            BTreeMap::from([(STUDENT, name("あおき"))])
        );

        // 時間と組み合わさる
        let list = use_case(sessions.clone(), RecordingLookup::default())
            .execute(
                OverdueSessionCriteria::new(5)
                    .unwrap()
                    .with_coach_id(Some(COACH)),
            )
            .await
            .unwrap();
        assert_eq!((list.criteria.hours(), ids(&list)), (5, vec![3]));

        // セッションのないコーチ・ない ID は空の一覧で、表示名を問い合わせない
        let lookup = RecordingLookup::default();
        let list = use_case(sessions, lookup.clone())
            .execute(
                OverdueSessionCriteria::default().with_coach_id(Some(AccountId::from_u128(99))),
            )
            .await
            .unwrap();
        assert!(list.sessions.is_empty());
        assert!(lookup.coaches_asked.lock().unwrap().is_empty());
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
    async fn no_overdue_sessions_with_a_sport_does_not_ask_for_coach_accounts() {
        let lookup = RecordingLookup::default();
        let list = use_case(
            [session(1, STUDENT, COACH, now() - HOUR, None)],
            lookup.clone(),
        )
        .execute(OverdueSessionCriteria::default().with_sport(Some(Sport::Tennis)))
        .await
        .unwrap();
        assert!(list.sessions.is_empty());
        assert!(lookup.coach_accounts_asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn listing_does_not_change_the_sessions() {
        let overdue = session(1, STUDENT, COACH, now() - 5 * HOUR, None);
        let repository = InMemoryLiveLessonSessionRepository::with_sessions([overdue.clone()]);
        ListOverdueLiveLessonSessions::new(
            repository.clone(),
            RecordingLookup::default(),
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
