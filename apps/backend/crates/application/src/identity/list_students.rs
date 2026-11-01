use std::collections::BTreeSet;

use domain::identity::AccountId;

use super::{
    AccountRepository, ListStudentsQuery, StudentContractLookup, StudentLastBookingLookup,
    StudentOverview,
};
use crate::{Unavailable, scheduling::calendar_day};

/// 生徒の一覧のユースケース（admin-web の `listStudents`。本部だけが呼ぶ）。照会だけで、何も保存しない。
///
/// 生徒のアカウントを登録の新しい順（同じ時刻は ID の順）に並べ、受講権の契約種別と契約プランを entitlement に
/// （[`StudentContractLookup`]）、最後に予約した時刻を scheduling に（[`StudentLastBookingLookup`]）問い合わせて添える。
/// 条件（[`ListStudentsQuery`]）があれば、表示名の検索語に部分一致する・そのスポーツの・その日付（日本時間。
/// [`calendar_day`] で区切る）の始まり以降に予約を作っていない生徒だけに絞る。条件がなければすべての生徒。
/// 誰が呼べるか（本部だけ）は境界（interface）が確かめる。
pub struct ListStudents<R, C, B> {
    accounts: R,
    contracts: C,
    bookings: B,
}

impl<R, C, B> ListStudents<R, C, B>
where
    R: AccountRepository,
    C: StudentContractLookup,
    B: StudentLastBookingLookup,
{
    pub fn new(accounts: R, contracts: C, bookings: B) -> Self {
        Self {
            accounts,
            contracts,
            bookings,
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
        let student_ids: BTreeSet<AccountId> =
            students.iter().map(|(account, _)| account.id()).collect();
        if student_ids.is_empty() {
            return Ok(Vec::new());
        }
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
        Ok(students
            .into_iter()
            .map(|(account, sport)| StudentOverview {
                student_id: account.id(),
                display_name: account.display_name().clone(),
                sport,
                contract: contracts.get(&account.id()).copied(),
                last_booked_at: last_booked_at.get(&account.id()).copied(),
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
        profile::{DisplayName, DisplayNameQuery},
    };

    use super::*;
    use crate::{identity::StudentContract, in_memory::InMemoryAccountRepository};

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
        let use_case = ListStudents::new(accounts().await, contracts, bookings);

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
                    s.registered_at,
                )
            })
            .collect();
        assert_eq!(
            listed,
            [
                (2, Sport::Swimming, None, Some(at(500)), at(300)),
                (5, Sport::Soccer, None, None, at(200)),
                (6, Sport::Golf, None, None, at(200)),
                (1, Sport::Tennis, Some(MONTHLY_PLUS), None, at(100)),
            ]
        );
        assert_eq!(students[0].display_name.as_str(), "なまえ2");

        let ids: BTreeSet<AccountId> = [1, 2, 5, 6].map(AccountId::from_u128).into();
        assert_eq!(
            *use_case.contracts.asked.lock().unwrap(),
            std::slice::from_ref(&ids),
            "生徒の ID だけをまとめて 1 回で問い合わせる"
        );
        assert_eq!(*use_case.bookings.asked.lock().unwrap(), [ids]);
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
        let use_case =
            ListStudents::new(accounts, FakeContracts::default(), FakeBookings::default());

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
        let use_case =
            ListStudents::new(accounts, FakeContracts::default(), FakeBookings::default());

        assert!(
            use_case
                .execute(&ListStudentsQuery::default())
                .await
                .unwrap()
                .is_empty()
        );
        assert!(use_case.contracts.asked.lock().unwrap().is_empty());
        assert!(use_case.bookings.asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn unavailable_lookup_is_an_error() {
        let use_case = ListStudents::new(accounts().await, Broken, FakeBookings::default());
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
        let use_case = ListStudents::new(accounts().await, contracts, DatedBookings);
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
        let use_case = ListStudents::new(accounts().await, FakeContracts::default(), DatedBookings);
        let nov1 = time::Date::from_calendar_date(2026, time::Month::November, 1).unwrap();
        let query = |sport| ListStudentsQuery {
            name: DisplayNameQuery::new("なまえ").unwrap(),
            sport: Some(sport),
            not_booked_since: Some(nov1),
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
}
