use std::collections::{BTreeMap, BTreeSet};

use domain::{
    DomainError,
    catalog::{LessonMenuId, Sport},
    identity::AccountId,
    live_lesson::SessionHistoryCriteria,
    scheduling::BookingId,
};
use time::Date;

use super::{
    LiveLessonSessionHistory, LiveLessonSessionRepository, SessionBookingLessonMenuLookup,
    SessionCoachAccountLookup, SessionCoachDisplayNameLookup, SessionStudentDisplayNameLookup,
};
use crate::{Unavailable, scheduling::calendar_day};

/// 本部が期間とコーチ・生徒・競技・レッスンメニューで見る通話セッションの記録の一覧のユースケース（admin-web の `listLiveLessonSessions`。本部だけが
/// 呼ぶ）。問い合わせを受けたときに、ライブレッスンがいつ始まりいつ終わったかを引くための照会で、何も保存しない。
///
/// 日付（日本時間）の `from` から `to` まで（両端を含む）に開始したセッションを、終了の有無を問わず読む。期間の規則
/// （`to` が `from` より前でない・[`SessionHistoryCriteria::MAX_DAYS`] 日まで）と選び方・並べ方（開始の新しい順）は
/// domain の [`SessionHistoryCriteria`]。日付の区切りは予約の一覧と同じ [`calendar_day`]。`coach_id` があればその
/// コーチのセッションだけ（いないコーチなら空の一覧）、`student_id` があればその生徒の予約が結び付いたセッションだけ
/// （グループレッスンでは開始した予約の生徒に限らない。いない生徒・生徒でない ID なら空の一覧）。`sport` があれば、選んだ
/// セッションのコーチのアカウント（[`SessionCoachAccountLookup`]）を引き、[`SessionHistoryCriteria::matches_coach_sport`]
/// に合うセッションだけを残す（並べ方は変わらない。`listOverdueLiveLessonSessions` と同じ）。`lesson_menu_id` があれば、
/// 選んだセッションに結び付いた予約の予約した時点のメニュー（[`SessionBookingLessonMenuLookup`]）を引き、
/// [`SessionHistoryCriteria::matches_lesson_menu`] に合うセッションだけを残す（ないメニューなら空の一覧）。各セッションには、コーチと開始した予約の生徒の表示名
/// （[`SessionCoachDisplayNameLookup`]・[`SessionStudentDisplayNameLookup`]）を引いて添える。誰が呼べるか（本部だけ）は
/// 境界（interface）が確かめる。
pub struct ListLiveLessonSessions<L, A, D, S, M> {
    sessions: L,
    coach_accounts: A,
    coach_names: D,
    student_names: S,
    booking_menus: M,
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
    /// あればこの生徒の予約が結び付いたセッションだけ
    pub student_id: Option<AccountId>,
    /// あればこの競技を提供するコーチのセッションだけ
    pub sport: Option<Sport>,
    /// あればこのレッスンメニューの予約が結び付いたセッションだけ
    pub lesson_menu_id: Option<LessonMenuId>,
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

impl<L, A, D, S, M> ListLiveLessonSessions<L, A, D, S, M>
where
    L: LiveLessonSessionRepository,
    A: SessionCoachAccountLookup,
    D: SessionCoachDisplayNameLookup,
    S: SessionStudentDisplayNameLookup,
    M: SessionBookingLessonMenuLookup,
{
    pub fn new(
        sessions: L,
        coach_accounts: A,
        coach_names: D,
        student_names: S,
        booking_menus: M,
    ) -> Self {
        Self {
            sessions,
            coach_accounts,
            coach_names,
            student_names,
            booking_menus,
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
            query.student_id,
        )?
        .with_sport(query.sport)
        .with_lesson_menu(query.lesson_menu_id);
        let found = self
            .sessions
            .find_started_between(
                criteria.starts_at(),
                criteria.ends_at(),
                criteria.coach_id(),
                criteria.student_id(),
            )
            .await?;
        let mut sessions = criteria.select(found);
        if criteria.lesson_menu_id().is_some() && !sessions.is_empty() {
            let booking_ids: BTreeSet<BookingId> = sessions
                .iter()
                .flat_map(|s| s.bookings().iter().map(|b| b.booking_id()))
                .collect();
            let menus = self
                .booking_menus
                .find_booking_lesson_menus(&booking_ids)
                .await?;
            sessions.retain(|session| criteria.matches_lesson_menu(session, &menus));
        }
        if criteria.sport().is_some() && !sessions.is_empty() {
            let coach_ids: BTreeSet<AccountId> = sessions.iter().map(|s| s.coach_id()).collect();
            let coaches = self.coach_accounts.find_coach_accounts(&coach_ids).await?;
            sessions
                .retain(|session| criteria.matches_coach_sport(coaches.get(&session.coach_id())));
        }

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
        identity::{Account, Email, Role},
        live_lesson::{ConnectionQuality, LiveLessonSession, SessionBooking, SessionId},
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

    const MENU: LessonMenuId = LessonMenuId::from_u128(50);
    const OTHER_MENU: LessonMenuId = LessonMenuId::from_u128(51);

    /// `COACH`（テニス・ゴルフ）と `OTHER_COACH`（サッカー）のアカウント、`COACH` と `STUDENT` の表示名、予約の
    /// メニュー（予約 101・105・107 は `MENU`、103 は `OTHER_MENU`、ほかはメニューのない予約）だけを知っていて、
    /// 問い合わせた ID を記録する。
    #[derive(Clone, Default)]
    struct RecordingLookup {
        booking_menus_asked: Arc<Mutex<Vec<BTreeSet<BookingId>>>>,
        coach_accounts_asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
        coaches_asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
        students_asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
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

    impl SessionBookingLessonMenuLookup for RecordingLookup {
        async fn find_booking_lesson_menus(
            &self,
            booking_ids: &BTreeSet<BookingId>,
        ) -> Result<BTreeMap<BookingId, LessonMenuId>, Unavailable> {
            self.booking_menus_asked
                .lock()
                .unwrap()
                .push(booking_ids.clone());
            Ok(booking_ids
                .iter()
                .filter_map(|id| match id.as_u128() {
                    101 | 105 | 107 => Some((*id, MENU)),
                    103 => Some((*id, OTHER_MENU)),
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

    type UseCase = ListLiveLessonSessions<
        InMemoryLiveLessonSessionRepository,
        RecordingLookup,
        RecordingLookup,
        RecordingLookup,
        RecordingLookup,
    >;

    fn use_case_with(sessions: Vec<LiveLessonSession>, lookup: RecordingLookup) -> UseCase {
        ListLiveLessonSessions::new(
            InMemoryLiveLessonSessionRepository::with_sessions(sessions),
            lookup.clone(),
            lookup.clone(),
            lookup.clone(),
            lookup,
        )
    }

    fn use_case(lookup: RecordingLookup) -> UseCase {
        use_case_with(sessions(), lookup)
    }

    fn query(from: u8, to: u8, coach_id: Option<AccountId>) -> ListLiveLessonSessionsQuery {
        ListLiveLessonSessionsQuery {
            from: date(Month::November, from),
            to: date(Month::November, to),
            coach_id,
            student_id: None,
            sport: None,
            lesson_menu_id: None,
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
    async fn the_sport_of_the_coach_narrows_the_sessions() {
        // アカウントの引けないコーチのセッション（11 月 2 日）
        let unknown_coach = AccountId::from_u128(9);
        let sessions: Vec<LiveLessonSession> = sessions()
            .into_iter()
            .chain([session(6, STUDENT, unknown_coach, at(2, 12), None)])
            .collect();
        let lookup = RecordingLookup::default();
        let history = use_case_with(sessions.clone(), lookup.clone())
            .execute(ListLiveLessonSessionsQuery {
                sport: Some(Sport::Golf),
                ..query(1, 3, None)
            })
            .await
            .unwrap();
        assert_eq!(
            ids(&history),
            [4, 1],
            "ほかの競技のコーチ・アカウントの引けないコーチ・期間の外は出ない。開始の新しい順のまま"
        );
        assert_eq!(
            *lookup.coach_accounts_asked.lock().unwrap(),
            [BTreeSet::from([COACH, OTHER_COACH, unknown_coach])],
            "期間で選んだ後のセッションのコーチを重ねずに 1 回で問い合わせる"
        );
        assert_eq!(
            *lookup.coaches_asked.lock().unwrap(),
            [BTreeSet::from([COACH])],
            "競技で絞った後のセッションのコーチだけ表示名を問い合わせる"
        );

        for (sport, coach_id, student_id, expected) in [
            (Sport::Tennis, None, None, vec![4, 1]),
            (Sport::Soccer, None, None, vec![3]),
            // コーチ・生徒の指定と組み合わさる
            (Sport::Soccer, Some(OTHER_COACH), None, vec![3]),
            (Sport::Soccer, Some(COACH), None, vec![]),
            (Sport::Golf, None, Some(STUDENT), vec![4, 1]),
            (Sport::Golf, None, Some(OTHER_STUDENT), vec![]),
            // 該当するセッションのない競技は空の一覧
            (Sport::Swimming, None, None, vec![]),
        ] {
            let history = use_case_with(sessions.clone(), RecordingLookup::default())
                .execute(ListLiveLessonSessionsQuery {
                    student_id,
                    sport: Some(sport),
                    ..query(1, 3, coach_id)
                })
                .await
                .unwrap();
            assert_eq!(
                ids(&history),
                expected,
                "{sport:?} {coach_id:?} {student_id:?}"
            );
        }

        // 競技で 1 件も残らなければ、表示名を問い合わせない
        let lookup = RecordingLookup::default();
        let history = use_case_with(sessions, lookup.clone())
            .execute(ListLiveLessonSessionsQuery {
                sport: Some(Sport::Baseball),
                ..query(1, 3, None)
            })
            .await
            .unwrap();
        assert!(history.sessions.is_empty());
        assert!(lookup.coaches_asked.lock().unwrap().is_empty());
        assert!(lookup.students_asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn without_sport_the_coach_accounts_are_not_asked() {
        let lookup = RecordingLookup::default();
        use_case(lookup.clone())
            .execute(query(1, 3, None))
            .await
            .unwrap();
        assert!(lookup.coach_accounts_asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_lesson_menu_of_the_linked_bookings_narrows_the_sessions() {
        // 通話を始めた予約（106）はメニューがなく、結び付いた予約（107）が MENU のグループレッスン（11 月 2 日）
        let group = LiveLessonSession::restore(
            SessionId::from_u128(6),
            BookingId::from_u128(106),
            STUDENT,
            COACH,
            vec![SessionBooking::new(
                BookingId::from_u128(107),
                OTHER_STUDENT,
            )],
            at(2, 12),
            None,
        )
        .unwrap();
        let sessions: Vec<LiveLessonSession> = sessions().into_iter().chain([group]).collect();
        let lookup = RecordingLookup::default();
        let history = use_case_with(sessions.clone(), lookup.clone())
            .execute(ListLiveLessonSessionsQuery {
                lesson_menu_id: Some(MENU),
                ..query(1, 3, None)
            })
            .await
            .unwrap();
        assert_eq!(
            ids(&history),
            [6, 1],
            "ほかのメニュー・メニューのない予約・期間の外（予約 105）は出ない。開始の新しい順のまま"
        );
        assert_eq!(
            *lookup.booking_menus_asked.lock().unwrap(),
            [[101, 103, 104, 106, 107].map(BookingId::from_u128).into()],
            "期間で選んだ後のセッションに結び付いた予約を重ねずに 1 回で問い合わせる"
        );
        assert_eq!(
            *lookup.students_asked.lock().unwrap(),
            [BTreeSet::from([STUDENT])],
            "メニューで絞った後のセッションの生徒だけ表示名を問い合わせる"
        );

        for (lesson_menu_id, coach_id, student_id, sport, expected) in [
            (OTHER_MENU, None, None, None, vec![3]),
            // コーチ・生徒・競技の指定と組み合わさる
            (MENU, Some(OTHER_COACH), None, None, vec![]),
            (MENU, None, Some(OTHER_STUDENT), None, vec![6]),
            (MENU, None, None, Some(Sport::Soccer), vec![]),
            (OTHER_MENU, None, None, Some(Sport::Soccer), vec![3]),
            // ないメニューは空の一覧
            (LessonMenuId::from_u128(99), None, None, None, vec![]),
        ] {
            let history = use_case_with(sessions.clone(), RecordingLookup::default())
                .execute(ListLiveLessonSessionsQuery {
                    student_id,
                    sport,
                    lesson_menu_id: Some(lesson_menu_id),
                    ..query(1, 3, coach_id)
                })
                .await
                .unwrap();
            assert_eq!(
                ids(&history),
                expected,
                "{lesson_menu_id:?} {coach_id:?} {student_id:?} {sport:?}"
            );
        }
    }

    #[tokio::test]
    async fn without_lesson_menu_the_booking_menus_are_not_asked() {
        let lookup = RecordingLookup::default();
        use_case(lookup.clone())
            .execute(query(1, 3, None))
            .await
            .unwrap();
        assert!(lookup.booking_menus_asked.lock().unwrap().is_empty());
        // 期間に 1 件もなければ、メニューを指定しても問い合わせない
        let lookup = RecordingLookup::default();
        let history = use_case(lookup.clone())
            .execute(ListLiveLessonSessionsQuery {
                lesson_menu_id: Some(MENU),
                ..query(10, 12, None)
            })
            .await
            .unwrap();
        assert!(history.sessions.is_empty());
        assert!(lookup.booking_menus_asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_student_narrows_the_sessions_including_group_lessons_they_joined() {
        // 通話を始めたのは STUDENT で、OTHER_STUDENT の予約も結び付いたグループレッスン（11 月 2 日）
        let group = LiveLessonSession::restore(
            SessionId::from_u128(6),
            BookingId::from_u128(106),
            STUDENT,
            COACH,
            vec![SessionBooking::new(
                BookingId::from_u128(107),
                OTHER_STUDENT,
            )],
            at(2, 12),
            None,
        )
        .unwrap();
        let use_case = use_case_with(
            sessions().into_iter().chain([group]).collect(),
            RecordingLookup::default(),
        );
        for (student_id, coach_id, expected) in [
            (STUDENT, None, vec![4, 6, 1]),
            (OTHER_STUDENT, None, vec![6, 3]),
            // コーチの指定と組み合わさる
            (OTHER_STUDENT, Some(OTHER_COACH), vec![3]),
            // 生徒でない ID・ない ID は空の一覧
            (COACH, None, vec![]),
            (AccountId::from_u128(99), None, vec![]),
        ] {
            let history = use_case
                .execute(ListLiveLessonSessionsQuery {
                    student_id: Some(student_id),
                    ..query(1, 3, coach_id)
                })
                .await
                .unwrap();
            assert_eq!(ids(&history), expected, "{student_id:?} {coach_id:?}");
        }
    }

    #[tokio::test]
    async fn a_single_day_is_from_midnight_to_midnight_in_japan() {
        let history = use_case(RecordingLookup::default())
            .execute(ListLiveLessonSessionsQuery {
                from: date(Month::October, 31),
                to: date(Month::October, 31),
                coach_id: None,
                student_id: None,
                sport: None,
                lesson_menu_id: None,
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
                    student_id: None,
                    sport: None,
                    lesson_menu_id: None,
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
                    student_id: None,
                    sport: None,
                    lesson_menu_id: None,
                })
                .await
                .is_ok()
        );
    }
}
