use std::collections::BTreeSet;

use domain::{identity::AccountId, messaging::UnansweredStudentFilter};

use super::{
    AccountRepository, ListStudentsQuery, StudentCoachBookingLookup, StudentContractLookup,
    StudentLastBookingLookup, StudentOverview, StudentRecentBookingCountLookup,
    StudentUnansweredMessageThreadCountsLookup,
};
use crate::{Unavailable, clock::Clock, scheduling::calendar_day};

/// 生徒の一覧のユースケース（admin-web の `listStudents`。本部だけが呼ぶ）。照会だけで、何も保存しない。
///
/// 生徒のアカウントを登録の新しい順（同じ時刻は ID の順）に並べ、受講権の契約種別と契約プランを entitlement に
/// （[`StudentContractLookup`]）、最後に予約した時刻を scheduling に（[`StudentLastBookingLookup`]）問い合わせて添える。
/// 直近の予約の数は、時計の今の時刻で scheduling に問い合わせて（[`StudentRecentBookingCountLookup`]）添える（並べ方・絞り込みには
/// 使わない）。
/// 条件（[`ListStudentsQuery`]）があれば、表示名の検索語に部分一致する・そのスポーツの・そのコーチを予約したことがある
/// （scheduling に [`StudentCoachBookingLookup`] で問い合わせる）・その日付（日本時間。[`calendar_day`] で区切る）の
/// 始まり以降に予約を作っていない生徒・返事を待っているメッセージのある（時計の今の時刻で messaging に
/// [`StudentUnansweredMessageThreadCountsLookup`] で問い合わせる。数え方は domain の
/// [`domain::messaging::StudentUnansweredMessageThreadCount::tally`] で、判定は [`UnansweredStudentFilter`]。絞るときだけ
/// 問い合わせる）生徒だけに絞る。絞っても並べ方は変えない。条件がなければすべての生徒。
/// 誰が呼べるか（本部だけ）は境界（interface）が確かめる。
pub struct ListStudents<R, C, B, K, N, U, L> {
    accounts: R,
    contracts: C,
    bookings: B,
    coach_bookings: K,
    recent_bookings: N,
    unanswered_threads: U,
    clock: L,
}

impl<R, C, B, K, N, U, L> ListStudents<R, C, B, K, N, U, L>
where
    R: AccountRepository,
    C: StudentContractLookup,
    B: StudentLastBookingLookup,
    K: StudentCoachBookingLookup,
    N: StudentRecentBookingCountLookup,
    U: StudentUnansweredMessageThreadCountsLookup,
    L: Clock,
{
    pub fn new(
        accounts: R,
        contracts: C,
        bookings: B,
        coach_bookings: K,
        recent_bookings: N,
        unanswered_threads: U,
        clock: L,
    ) -> Self {
        Self {
            accounts,
            contracts,
            bookings,
            coach_bookings,
            recent_bookings,
            unanswered_threads,
            clock,
        }
    }

    pub async fn execute(
        &self,
        query: &ListStudentsQuery,
    ) -> Result<Vec<StudentOverview>, Unavailable> {
        let mut students = self
            .accounts
            .list_students(query.name.as_ref())
            .await?
            .into_iter()
            .map(|account| {
                let sport = account
                    .as_student()
                    .ok_or_else(|| Unavailable::new("生徒の一覧に生徒でないアカウントがある"))?
                    .sport();
                Ok((account, sport))
            })
            .collect::<Result<Vec<_>, Unavailable>>()?;
        if let Some(wanted) = query.sport {
            students.retain(|(_, sport)| *sport == wanted);
        }
        if students.is_empty() {
            return Ok(Vec::new());
        }
        if let Some(coach_id) = query.coach_id {
            let booked = self
                .coach_bookings
                .find_students_who_booked_coach(coach_id)
                .await?;
            students.retain(|(account, _)| booked.contains(&account.id()));
            if students.is_empty() {
                return Ok(Vec::new());
            }
        }
        let unanswered = UnansweredStudentFilter::new(query.has_unanswered);
        // 絞らないときは messaging に問い合わせない（今までどおり）
        if unanswered.has_unanswered() {
            let student_ids: BTreeSet<AccountId> =
                students.iter().map(|(account, _)| account.id()).collect();
            let counts = self
                .unanswered_threads
                .find_unanswered_message_thread_counts(&student_ids, self.clock.now())
                .await?;
            // ポートは聞いた生徒をすべて返す。返さなかった生徒は 0 件として扱う（載せない）
            students.retain(|(account, _)| {
                counts
                    .get(&account.id())
                    .is_some_and(|count| unanswered.matches(count))
            });
            if students.is_empty() {
                return Ok(Vec::new());
            }
        }
        let student_ids: BTreeSet<AccountId> =
            students.iter().map(|(account, _)| account.id()).collect();
        let last_booked_at = self.bookings.find_last_booked_at(&student_ids).await?;
        if let Some(since) = query.not_booked_since.map(calendar_day::start) {
            students.retain(|(account, _)| {
                last_booked_at
                    .get(&account.id())
                    .is_none_or(|booked_at| *booked_at < since)
            });
            if students.is_empty() {
                return Ok(Vec::new());
            }
        }
        let student_ids: BTreeSet<AccountId> =
            students.iter().map(|(account, _)| account.id()).collect();
        let contracts = self.contracts.find_student_contracts(&student_ids).await?;
        let recent_booking_counts = self
            .recent_bookings
            .count_recent_bookings(&student_ids, self.clock.now())
            .await?;
        Ok(students
            .into_iter()
            .map(|(account, sport)| StudentOverview {
                student_id: account.id(),
                display_name: account.display_name().clone(),
                sport,
                contract: contracts.get(&account.id()).copied(),
                last_booked_at: last_booked_at.get(&account.id()).copied(),
                recent_booking_count: recent_booking_counts
                    .get(&account.id())
                    .copied()
                    .unwrap_or(0),
                registered_at: account.registered_at(),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::Mutex,
        time::{Duration, SystemTime},
    };

    use domain::{
        catalog::Sport,
        entitlement::{ContractType, SubscriptionPlan},
        identity::{Account, Email, Role},
        messaging::{
            MessageBody, MessageContent, MessageId, MessageThread, MessageThreadId,
            StudentUnansweredMessageThreadCount,
        },
        profile::{DisplayName, DisplayNameQuery},
    };

    use super::*;
    use crate::{
        identity::StudentContract,
        in_memory::{FixedClock, InMemoryAccountRepository},
    };

    /// 照会した時刻（時計の今の時刻）。
    const NOW: u64 = 1_793_404_800;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn account(id: u128, role: Role, sports: &[Sport], registered_at: u64) -> Account {
        Account::new(
            AccountId::from_u128(id),
            Email::new(format!("user{id}@example.com")).unwrap(),
            DisplayName::new(format!("なまえ{id}")).unwrap(),
            role,
            sports.iter().copied(),
            at(registered_at),
        )
        .unwrap()
    }

    const MONTHLY_PLUS: StudentContract = StudentContract {
        contract_type: ContractType::Monthly,
        plan: Some(SubscriptionPlan::Plus),
    };

    /// 生徒 1 は月額（plus）の契約がある。問い合わせた ID を記録する。
    #[derive(Default)]
    struct FakeContracts {
        asked: Mutex<Vec<BTreeSet<AccountId>>>,
    }

    impl StudentContractLookup for FakeContracts {
        async fn find_student_contracts(
            &self,
            student_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, StudentContract>, Unavailable> {
            self.asked.lock().unwrap().push(student_ids.clone());
            Ok([(AccountId::from_u128(1), MONTHLY_PLUS)]
                .into_iter()
                .filter(|(id, _)| student_ids.contains(id))
                .collect())
        }
    }

    /// 生徒 2 は時刻 500 に最後に予約した。問い合わせた ID を記録する。
    #[derive(Default)]
    struct FakeBookings {
        asked: Mutex<Vec<BTreeSet<AccountId>>>,
    }

    impl StudentLastBookingLookup for FakeBookings {
        async fn find_last_booked_at(
            &self,
            student_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, SystemTime>, Unavailable> {
            self.asked.lock().unwrap().push(student_ids.clone());
            Ok([(AccountId::from_u128(2), at(500))]
                .into_iter()
                .filter(|(id, _)| student_ids.contains(id))
                .collect())
        }
    }

    /// コーチ 3 の枠を生徒 2・5・6 が予約したことがある。問い合わせたコーチの ID を記録する。
    #[derive(Default)]
    struct FakeCoachBookings {
        asked: Mutex<Vec<AccountId>>,
    }

    impl StudentCoachBookingLookup for FakeCoachBookings {
        async fn find_students_who_booked_coach(
            &self,
            coach_id: AccountId,
        ) -> Result<BTreeSet<AccountId>, Unavailable> {
            self.asked.lock().unwrap().push(coach_id);
            Ok(if coach_id == AccountId::from_u128(3) {
                [2, 5, 6].map(AccountId::from_u128).into()
            } else {
                BTreeSet::new()
            })
        }
    }

    /// 生徒 1 は直近の予約が 3 件、生徒 6 は 1 件。問い合わせた ID と時刻を記録する。
    #[derive(Default)]
    struct FakeRecentBookings {
        asked: Mutex<Vec<(BTreeSet<AccountId>, SystemTime)>>,
    }

    impl StudentRecentBookingCountLookup for FakeRecentBookings {
        async fn count_recent_bookings(
            &self,
            student_ids: &BTreeSet<AccountId>,
            now: SystemTime,
        ) -> Result<BTreeMap<AccountId, u32>, Unavailable> {
            self.asked.lock().unwrap().push((student_ids.clone(), now));
            Ok([(1, 3), (6, 1)]
                .into_iter()
                .map(|(id, count)| (AccountId::from_u128(id), count))
                .filter(|(id, _)| student_ids.contains(id))
                .collect())
        }
    }

    /// 生徒 2 は返信のないまま既定の時間の過ぎたスレッドが 2 件、生徒 6 は 1 件、生徒 5 は返信を待っていないスレッドだけ
    /// （0 件）。聞いた生徒をすべて返す（生徒 1 はスレッドがなく 0 件）。問い合わせた ID と時刻を記録する。
    #[derive(Default)]
    struct FakeUnansweredThreads {
        asked: Mutex<Vec<(BTreeSet<AccountId>, SystemTime)>>,
    }

    impl StudentUnansweredMessageThreadCountsLookup for FakeUnansweredThreads {
        async fn find_unanswered_message_thread_counts(
            &self,
            student_ids: &BTreeSet<AccountId>,
            now: SystemTime,
        ) -> Result<BTreeMap<AccountId, StudentUnansweredMessageThreadCount>, Unavailable> {
            self.asked.lock().unwrap().push((student_ids.clone(), now));
            let threads = [
                unanswered_thread(1, 2, now),
                unanswered_thread(2, 2, now),
                unanswered_thread(3, 6, now),
                answered_thread(4, 5, now),
            ];
            Ok(student_ids
                .iter()
                .map(|&id| {
                    (
                        id,
                        StudentUnansweredMessageThreadCount::tally(id, &threads, now),
                    )
                })
                .collect())
        }
    }

    /// 生徒 `student` が `now` の 24 時間前に送り、コーチ 3 がまだ返していないスレッド。
    fn unanswered_thread(id: u128, student: u128, now: SystemTime) -> MessageThread {
        let mut thread = MessageThread::open(
            MessageThreadId::from_u128(id),
            AccountId::from_u128(student),
            AccountId::from_u128(3),
            now - Duration::from_secs(240 * 3600),
        )
        .unwrap();
        thread
            .send(
                MessageId::from_u128(id * 100),
                AccountId::from_u128(student),
                MessageContent::Text(MessageBody::new("質問です").unwrap()),
                true,
                now - Duration::from_secs(24 * 3600),
            )
            .unwrap();
        thread
    }

    /// 生徒 `student` が送り、コーチ 3 が `now` の 1 時間前に返したスレッド。
    fn answered_thread(id: u128, student: u128, now: SystemTime) -> MessageThread {
        let mut thread = unanswered_thread(id, student, now);
        thread
            .send(
                MessageId::from_u128(id * 100 + 1),
                AccountId::from_u128(3),
                MessageContent::Text(MessageBody::new("お答えします").unwrap()),
                true,
                now - Duration::from_secs(3600),
            )
            .unwrap();
        thread
    }

    /// 取り出せない保存先の偽物。
    struct Broken;

    impl StudentContractLookup for Broken {
        async fn find_student_contracts(
            &self,
            _student_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, StudentContract>, Unavailable> {
            Err(Unavailable::new("壊れている"))
        }
    }

    async fn accounts() -> InMemoryAccountRepository {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, Role::Student, &[Sport::Tennis], 100),
            account(2, Role::Student, &[Sport::Swimming], 300),
            account(3, Role::Coach, &[Sport::Tennis], 400),
            account(4, Role::Headquarters, &[], 500),
            // 同じ登録時刻は ID の順
            account(6, Role::Student, &[Sport::Golf], 200),
            account(5, Role::Student, &[Sport::Soccer], 200),
        ] {
            accounts.insert(account).await.unwrap();
        }
        accounts
    }

    #[tokio::test]
    async fn lists_only_students_newest_first_with_contract_and_last_booking() {
        let contracts = FakeContracts::default();
        let bookings = FakeBookings::default();
        let use_case = ListStudents::new(
            accounts().await,
            contracts,
            bookings,
            FakeCoachBookings::default(),
            FakeRecentBookings::default(),
            FakeUnansweredThreads::default(),
            FixedClock(at(NOW)),
        );

        let students = use_case
            .execute(&ListStudentsQuery::default())
            .await
            .unwrap();
        let listed: Vec<_> = students
            .iter()
            .map(|s| {
                (
                    s.student_id.as_u128(),
                    s.sport,
                    s.contract,
                    s.last_booked_at,
                    s.recent_booking_count,
                    s.registered_at,
                )
            })
            .collect();
        assert_eq!(
            listed,
            [
                (2, Sport::Swimming, None, Some(at(500)), 0, at(300)),
                (5, Sport::Soccer, None, None, 0, at(200)),
                (6, Sport::Golf, None, None, 1, at(200)),
                (1, Sport::Tennis, Some(MONTHLY_PLUS), None, 3, at(100)),
            ],
            "直近の予約がない生徒は 0。直近の予約の数は並べ方を変えない"
        );
        assert_eq!(students[0].display_name.as_str(), "なまえ2");

        let ids: BTreeSet<AccountId> = [1, 2, 5, 6].map(AccountId::from_u128).into();
        assert_eq!(
            *use_case.contracts.asked.lock().unwrap(),
            std::slice::from_ref(&ids),
            "生徒の ID だけをまとめて 1 回で問い合わせる"
        );
        assert_eq!(
            *use_case.bookings.asked.lock().unwrap(),
            std::slice::from_ref(&ids)
        );
        assert_eq!(
            *use_case.recent_bookings.asked.lock().unwrap(),
            [(ids, at(NOW))],
            "直近の予約の数は生徒の ID をまとめて、時計の今の時刻で問い合わせる"
        );
        assert!(
            use_case.unanswered_threads.asked.lock().unwrap().is_empty(),
            "返事を待っているメッセージで絞らないときは messaging に問い合わせない"
        );
    }

    #[tokio::test]
    async fn query_lists_only_students_whose_display_name_contains_it() {
        let by_name = |name: DisplayNameQuery| ListStudentsQuery {
            name: Some(name),
            ..ListStudentsQuery::default()
        };
        let accounts = InMemoryAccountRepository::default();
        for (id, role, name, registered_at) in [
            (1, Role::Student, "Taro Yamada", 100),
            (2, Role::Student, "はなこ", 200),
            (3, Role::Student, "山田 次郎", 300),
            // 生徒でないアカウントは表示名が一致しても載せない
            (4, Role::Coach, "山田コーチ", 400),
        ] {
            let sports: &[Sport] = &[Sport::Tennis];
            let account = Account::new(
                AccountId::from_u128(id),
                Email::new(format!("user{id}@example.com")).unwrap(),
                DisplayName::new(name).unwrap(),
                role,
                sports.iter().copied(),
                at(registered_at),
            )
            .unwrap();
            accounts.insert(account).await.unwrap();
        }
        let use_case = ListStudents::new(
            accounts,
            FakeContracts::default(),
            FakeBookings::default(),
            FakeCoachBookings::default(),
            FakeRecentBookings::default(),
            FakeUnansweredThreads::default(),
            FixedClock(at(NOW)),
        );

        let listed = |students: Vec<StudentOverview>| -> Vec<u128> {
            students.iter().map(|s| s.student_id.as_u128()).collect()
        };
        // ASCII の英字の大文字・小文字は区別しない。登録の新しい順はそのまま
        let query = DisplayNameQuery::new("YAMADA").unwrap().unwrap();
        assert_eq!(
            listed(use_case.execute(&by_name(query)).await.unwrap()),
            [1]
        );
        let query = DisplayNameQuery::new("山田").unwrap().unwrap();
        assert_eq!(
            listed(use_case.execute(&by_name(query)).await.unwrap()),
            [3]
        );
        assert_eq!(
            *use_case.contracts.asked.lock().unwrap().last().unwrap(),
            [AccountId::from_u128(3)].into(),
            "一致した生徒の ID だけを問い合わせる"
        );

        let before = use_case.contracts.asked.lock().unwrap().len();
        let query = DisplayNameQuery::new("いない").unwrap().unwrap();
        assert!(use_case.execute(&by_name(query)).await.unwrap().is_empty());
        assert_eq!(
            use_case.contracts.asked.lock().unwrap().len(),
            before,
            "一致する生徒がいなければ他のコンテキストに問い合わせない"
        );
    }

    #[tokio::test]
    async fn no_students_is_an_empty_list_without_asking_other_contexts() {
        let accounts = InMemoryAccountRepository::default();
        accounts
            .insert(account(3, Role::Coach, &[Sport::Tennis], 100))
            .await
            .unwrap();
        let use_case = ListStudents::new(
            accounts,
            FakeContracts::default(),
            FakeBookings::default(),
            FakeCoachBookings::default(),
            FakeRecentBookings::default(),
            FakeUnansweredThreads::default(),
            FixedClock(at(NOW)),
        );

        assert!(
            use_case
                .execute(&ListStudentsQuery::default())
                .await
                .unwrap()
                .is_empty()
        );
        assert!(use_case.contracts.asked.lock().unwrap().is_empty());
        assert!(use_case.bookings.asked.lock().unwrap().is_empty());
        assert!(use_case.recent_bookings.asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn unavailable_lookup_is_an_error() {
        let use_case = ListStudents::new(
            accounts().await,
            Broken,
            FakeBookings::default(),
            FakeCoachBookings::default(),
            FakeRecentBookings::default(),
            FakeUnansweredThreads::default(),
            FixedClock(at(NOW)),
        );
        assert!(
            use_case
                .execute(&ListStudentsQuery::default())
                .await
                .is_err()
        );
    }

    /// 生徒 2 は時刻 500 に、生徒 5 は日本時間の 2026-11-01 の始まりちょうどに、生徒 6 はその 1 秒前に最後に予約した。
    struct DatedBookings;

    /// 2026-11-01T00:00:00+09:00（日本時間の 2026-11-01 の始まり）。
    const NOV1_JST: u64 = 1_793_458_800;

    impl StudentLastBookingLookup for DatedBookings {
        async fn find_last_booked_at(
            &self,
            student_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, SystemTime>, Unavailable> {
            Ok([(2, at(500)), (5, at(NOV1_JST)), (6, at(NOV1_JST - 1))]
                .into_iter()
                .map(|(id, booked_at)| (AccountId::from_u128(id), booked_at))
                .filter(|(id, _)| student_ids.contains(id))
                .collect())
        }
    }

    fn listed(students: &[StudentOverview]) -> Vec<u128> {
        students.iter().map(|s| s.student_id.as_u128()).collect()
    }

    #[tokio::test]
    async fn sport_lists_only_students_of_the_sport() {
        let use_case = ListStudents::new(
            accounts().await,
            FakeContracts::default(),
            FakeBookings::default(),
            FakeCoachBookings::default(),
            FakeRecentBookings::default(),
            FakeUnansweredThreads::default(),
            FixedClock(at(NOW)),
        );
        let query = |sport| ListStudentsQuery {
            sport: Some(sport),
            ..ListStudentsQuery::default()
        };

        let students = use_case.execute(&query(Sport::Swimming)).await.unwrap();
        assert_eq!(listed(&students), [2]);
        assert_eq!(
            students[0].last_booked_at,
            Some(at(500)),
            "絞り込んでも最後の予約を添える"
        );
        assert_eq!(
            *use_case.bookings.asked.lock().unwrap(),
            [[AccountId::from_u128(2)].into()],
            "そのスポーツの生徒の ID だけを問い合わせる"
        );
        assert_eq!(
            listed(&use_case.execute(&query(Sport::Tennis)).await.unwrap()),
            [1]
        );
        assert_eq!(
            listed(&use_case.execute(&query(Sport::Golf)).await.unwrap()),
            [6],
            "ゴルフの生徒も絞り込める"
        );

        let asked = use_case.bookings.asked.lock().unwrap().len();
        assert!(
            use_case
                .execute(&query(Sport::Baseball))
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            use_case.bookings.asked.lock().unwrap().len(),
            asked,
            "そのスポーツの生徒がいなければ他のコンテキストに問い合わせない"
        );
    }

    #[tokio::test]
    async fn not_booked_since_lists_students_whose_last_booking_is_before_the_date_or_none() {
        let contracts = FakeContracts::default();
        let use_case = ListStudents::new(
            accounts().await,
            contracts,
            DatedBookings,
            FakeCoachBookings::default(),
            FakeRecentBookings::default(),
            FakeUnansweredThreads::default(),
            FixedClock(at(NOW)),
        );
        let nov1 = time::Date::from_calendar_date(2026, time::Month::November, 1).unwrap();
        let query = ListStudentsQuery {
            not_booked_since: Some(nov1),
            ..ListStudentsQuery::default()
        };

        let students = use_case.execute(&query).await.unwrap();
        // 生徒 5 は日付の始まり（日本時間の 0 時）ちょうどに予約したので外れる。生徒 1 は一度も予約していない
        assert_eq!(listed(&students), [2, 6, 1]);
        assert_eq!(
            students
                .iter()
                .map(|s| s.last_booked_at)
                .collect::<Vec<_>>(),
            [Some(at(500)), Some(at(NOV1_JST - 1)), None]
        );
        assert_eq!(students[2].contract, Some(MONTHLY_PLUS));
        assert_eq!(
            *use_case.contracts.asked.lock().unwrap(),
            [[1, 2, 6].map(AccountId::from_u128).into()],
            "絞り込んだあとの生徒の ID だけを entitlement に問い合わせる"
        );

        let next_day = ListStudentsQuery {
            not_booked_since: nov1.next_day(),
            ..ListStudentsQuery::default()
        };
        assert_eq!(
            listed(&use_case.execute(&next_day).await.unwrap()),
            [2, 5, 6, 1],
            "翌日からなら日付の始まりちょうどに予約した生徒も載る"
        );
    }

    #[tokio::test]
    async fn conditions_are_combined() {
        let use_case = ListStudents::new(
            accounts().await,
            FakeContracts::default(),
            DatedBookings,
            FakeCoachBookings::default(),
            FakeRecentBookings::default(),
            FakeUnansweredThreads::default(),
            FixedClock(at(NOW)),
        );
        let nov1 = time::Date::from_calendar_date(2026, time::Month::November, 1).unwrap();
        let query = |sport| ListStudentsQuery {
            name: DisplayNameQuery::new("なまえ").unwrap(),
            sport: Some(sport),
            not_booked_since: Some(nov1),
            coach_id: None,
            has_unanswered: false,
        };

        assert_eq!(
            listed(&use_case.execute(&query(Sport::Golf)).await.unwrap()),
            [6]
        );
        let asked = use_case.contracts.asked.lock().unwrap().len();
        assert!(
            use_case
                .execute(&query(Sport::Soccer))
                .await
                .unwrap()
                .is_empty(),
            "サッカーの生徒 5 は日付の始まり以降に予約している"
        );
        assert_eq!(
            use_case.contracts.asked.lock().unwrap().len(),
            asked,
            "残る生徒がいなければ entitlement に問い合わせない"
        );
    }

    #[tokio::test]
    async fn coach_id_lists_only_students_who_booked_the_coach_in_the_same_order() {
        let use_case = ListStudents::new(
            accounts().await,
            FakeContracts::default(),
            FakeBookings::default(),
            FakeCoachBookings::default(),
            FakeRecentBookings::default(),
            FakeUnansweredThreads::default(),
            FixedClock(at(NOW)),
        );
        let query = |coach: u128| ListStudentsQuery {
            coach_id: Some(AccountId::from_u128(coach)),
            ..ListStudentsQuery::default()
        };

        let students = use_case.execute(&query(3)).await.unwrap();
        // 登録の新しい順はそのまま。予約したことのない生徒 1 は載らない
        assert_eq!(listed(&students), [2, 5, 6]);
        assert_eq!(
            students[0].last_booked_at,
            Some(at(500)),
            "絞り込んでも最後の予約を添える"
        );
        let ids: BTreeSet<AccountId> = [2, 5, 6].map(AccountId::from_u128).into();
        assert_eq!(
            *use_case.bookings.asked.lock().unwrap(),
            std::slice::from_ref(&ids),
            "予約したことがある生徒の ID だけを問い合わせる"
        );
        assert_eq!(
            *use_case.contracts.asked.lock().unwrap(),
            std::slice::from_ref(&ids)
        );
        assert_eq!(
            *use_case.recent_bookings.asked.lock().unwrap(),
            [(ids, at(NOW))],
            "絞り込んだあとの生徒の ID だけを直近の予約の数に問い合わせる"
        );
        assert_eq!(
            students
                .iter()
                .map(|s| s.recent_booking_count)
                .collect::<Vec<_>>(),
            [0, 0, 1]
        );

        // ないコーチ・コーチでない ID（本部 4・生徒 1）は空の一覧で、ほかのコンテキストに問い合わせない
        for coach in [9, 4, 1] {
            assert!(use_case.execute(&query(coach)).await.unwrap().is_empty());
        }
        assert_eq!(
            *use_case.coach_bookings.asked.lock().unwrap(),
            [3, 9, 4, 1].map(AccountId::from_u128)
        );
        assert_eq!(use_case.bookings.asked.lock().unwrap().len(), 1);
        assert_eq!(use_case.contracts.asked.lock().unwrap().len(), 1);
        assert_eq!(use_case.recent_bookings.asked.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn coach_id_is_combined_with_other_conditions() {
        let use_case = ListStudents::new(
            accounts().await,
            FakeContracts::default(),
            DatedBookings,
            FakeCoachBookings::default(),
            FakeRecentBookings::default(),
            FakeUnansweredThreads::default(),
            FixedClock(at(NOW)),
        );
        let nov1 = time::Date::from_calendar_date(2026, time::Month::November, 1).unwrap();
        let coach = Some(AccountId::from_u128(3));

        let not_booked_since = ListStudentsQuery {
            coach_id: coach,
            not_booked_since: Some(nov1),
            ..ListStudentsQuery::default()
        };
        assert_eq!(
            listed(&use_case.execute(&not_booked_since).await.unwrap()),
            [2, 6],
            "生徒 5 は日付の始まり以降に予約している"
        );

        let by_sport = |sport| ListStudentsQuery {
            coach_id: coach,
            sport: Some(sport),
            ..ListStudentsQuery::default()
        };
        assert_eq!(
            listed(&use_case.execute(&by_sport(Sport::Golf)).await.unwrap()),
            [6]
        );
        assert!(
            use_case
                .execute(&by_sport(Sport::Tennis))
                .await
                .unwrap()
                .is_empty(),
            "テニスの生徒 1 はコーチ 3 を予約したことがない"
        );
        let asked = use_case.coach_bookings.asked.lock().unwrap().len();
        assert!(
            use_case
                .execute(&by_sport(Sport::Baseball))
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            use_case.coach_bookings.asked.lock().unwrap().len(),
            asked,
            "そのスポーツの生徒がいなければ scheduling に問い合わせない"
        );

        let by_name = |name: &str| ListStudentsQuery {
            coach_id: coach,
            name: DisplayNameQuery::new(name).unwrap(),
            ..ListStudentsQuery::default()
        };
        assert_eq!(
            listed(&use_case.execute(&by_name("なまえ5")).await.unwrap()),
            [5]
        );
        assert!(
            use_case
                .execute(&by_name("なまえ1"))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn has_unanswered_lists_only_students_with_an_unanswered_thread_in_the_same_order() {
        let use_case = ListStudents::new(
            accounts().await,
            FakeContracts::default(),
            FakeBookings::default(),
            FakeCoachBookings::default(),
            FakeRecentBookings::default(),
            FakeUnansweredThreads::default(),
            FixedClock(at(NOW)),
        );
        let query = ListStudentsQuery {
            has_unanswered: true,
            ..ListStudentsQuery::default()
        };

        let students = use_case.execute(&query).await.unwrap();
        // 登録の新しい順はそのまま。返信を待っていないスレッドだけの生徒 5・スレッドのない生徒 1 は載らない
        assert_eq!(listed(&students), [2, 6]);
        assert_eq!(
            students[0].last_booked_at,
            Some(at(500)),
            "絞り込んでも最後の予約を添える"
        );
        assert_eq!(
            *use_case.unanswered_threads.asked.lock().unwrap(),
            [([1, 2, 5, 6].map(AccountId::from_u128).into(), at(NOW))],
            "生徒の ID をまとめて、時計の今の時刻で 1 回だけ問い合わせる"
        );
        let ids: BTreeSet<AccountId> = [2, 6].map(AccountId::from_u128).into();
        assert_eq!(
            *use_case.contracts.asked.lock().unwrap(),
            std::slice::from_ref(&ids),
            "絞り込んだあとの生徒の ID だけを entitlement に問い合わせる"
        );
        assert_eq!(
            *use_case.bookings.asked.lock().unwrap(),
            std::slice::from_ref(&ids)
        );

        let not_narrowed = ListStudentsQuery {
            has_unanswered: false,
            ..ListStudentsQuery::default()
        };
        assert_eq!(
            listed(&use_case.execute(&not_narrowed).await.unwrap()),
            [2, 5, 6, 1],
            "false なら絞らない"
        );
        assert_eq!(use_case.unanswered_threads.asked.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn has_unanswered_is_combined_with_other_conditions() {
        let use_case = ListStudents::new(
            accounts().await,
            FakeContracts::default(),
            DatedBookings,
            FakeCoachBookings::default(),
            FakeRecentBookings::default(),
            FakeUnansweredThreads::default(),
            FixedClock(at(NOW)),
        );
        let nov1 = time::Date::from_calendar_date(2026, time::Month::November, 1).unwrap();
        let unanswered = |query: ListStudentsQuery| ListStudentsQuery {
            has_unanswered: true,
            ..query
        };

        let by_sport = |sport| {
            unanswered(ListStudentsQuery {
                sport: Some(sport),
                ..ListStudentsQuery::default()
            })
        };
        assert_eq!(
            listed(&use_case.execute(&by_sport(Sport::Golf)).await.unwrap()),
            [6]
        );
        assert!(
            use_case
                .execute(&by_sport(Sport::Soccer))
                .await
                .unwrap()
                .is_empty(),
            "サッカーの生徒 5 は返信を待っていない"
        );

        let by_name = |name: &str| {
            unanswered(ListStudentsQuery {
                name: DisplayNameQuery::new(name).unwrap(),
                ..ListStudentsQuery::default()
            })
        };
        assert_eq!(
            listed(&use_case.execute(&by_name("なまえ2")).await.unwrap()),
            [2]
        );

        let with_coach = |coach: u128| {
            unanswered(ListStudentsQuery {
                coach_id: Some(AccountId::from_u128(coach)),
                ..ListStudentsQuery::default()
            })
        };
        assert_eq!(
            listed(&use_case.execute(&with_coach(3)).await.unwrap()),
            [2, 6],
            "コーチ 3 を予約した生徒 2・5・6 のうち返信を待っている生徒"
        );

        let not_booked_since = unanswered(ListStudentsQuery {
            not_booked_since: Some(nov1),
            ..ListStudentsQuery::default()
        });
        assert_eq!(
            listed(&use_case.execute(&not_booked_since).await.unwrap()),
            [2, 6]
        );
        let all = ListStudentsQuery {
            name: DisplayNameQuery::new("なまえ").unwrap(),
            sport: Some(Sport::Swimming),
            not_booked_since: Some(nov1),
            coach_id: Some(AccountId::from_u128(3)),
            has_unanswered: true,
        };
        assert_eq!(listed(&use_case.execute(&all).await.unwrap()), [2]);
        assert_eq!(
            use_case.unanswered_threads.asked.lock().unwrap().last(),
            Some(&([AccountId::from_u128(2)].into(), at(NOW))),
            "ほかの条件で絞ったあとの生徒の ID だけを問い合わせる"
        );

        let asked = use_case.unanswered_threads.asked.lock().unwrap().len();
        assert!(
            use_case
                .execute(&by_sport(Sport::Baseball))
                .await
                .unwrap()
                .is_empty()
        );
        assert!(use_case.execute(&with_coach(9)).await.unwrap().is_empty());
        assert_eq!(
            use_case.unanswered_threads.asked.lock().unwrap().len(),
            asked,
            "ほかの条件で残る生徒がいなければ messaging に問い合わせない"
        );
    }
}
