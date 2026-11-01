use std::collections::BTreeSet;

use domain::{identity::AccountId, profile::DisplayNameQuery};

use super::{AccountRepository, StudentContractLookup, StudentLastBookingLookup, StudentOverview};
use crate::Unavailable;

/// 生徒の一覧のユースケース（admin-web の `listStudents`。本部だけが呼ぶ）。照会だけで、何も保存しない。
///
/// 生徒のアカウント（検索語 [`DisplayNameQuery`] があれば表示名がそれに部分一致する生徒だけ）を登録の新しい順
/// （同じ時刻は ID の順）に並べ、受講権の契約種別と契約プランを entitlement に
/// （[`StudentContractLookup`]）、最後に予約した時刻を scheduling に（[`StudentLastBookingLookup`]）問い合わせて添える。
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
        name: Option<&DisplayNameQuery>,
    ) -> Result<Vec<StudentOverview>, Unavailable> {
        let students = self.accounts.list_students(name).await?;
        let student_ids: BTreeSet<AccountId> = students.iter().map(|a| a.id()).collect();
        if student_ids.is_empty() {
            return Ok(Vec::new());
        }
        let contracts = self.contracts.find_student_contracts(&student_ids).await?;
        let last_booked_at = self.bookings.find_last_booked_at(&student_ids).await?;
        students
            .into_iter()
            .map(|account| {
                let student = account
                    .as_student()
                    .ok_or_else(|| Unavailable::new("生徒の一覧に生徒でないアカウントがある"))?;
                Ok(StudentOverview {
                    student_id: account.id(),
                    display_name: account.display_name().clone(),
                    sport: student.sport(),
                    contract: contracts.get(&account.id()).copied(),
                    last_booked_at: last_booked_at.get(&account.id()).copied(),
                    registered_at: account.registered_at(),
                })
            })
            .collect()
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

        let students = use_case.execute(None).await.unwrap();
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
        assert_eq!(listed(use_case.execute(Some(&query)).await.unwrap()), [1]);
        let query = DisplayNameQuery::new("山田").unwrap().unwrap();
        assert_eq!(listed(use_case.execute(Some(&query)).await.unwrap()), [3]);
        assert_eq!(
            *use_case.contracts.asked.lock().unwrap().last().unwrap(),
            [AccountId::from_u128(3)].into(),
            "一致した生徒の ID だけを問い合わせる"
        );

        let before = use_case.contracts.asked.lock().unwrap().len();
        let query = DisplayNameQuery::new("いない").unwrap().unwrap();
        assert!(use_case.execute(Some(&query)).await.unwrap().is_empty());
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

        assert!(use_case.execute(None).await.unwrap().is_empty());
        assert!(use_case.contracts.asked.lock().unwrap().is_empty());
        assert!(use_case.bookings.asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn unavailable_lookup_is_an_error() {
        let use_case = ListStudents::new(accounts().await, Broken, FakeBookings::default());
        assert!(use_case.execute(None).await.is_err());
    }
}
