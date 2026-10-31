use domain::{catalog::Sport, identity::AccountId};

use crate::{Unavailable, identity::AccountRepository, notification::AnnouncementRecipientLookup};

/// notification の [`AnnouncementRecipientLookup`] の profile による実装。アカウントの保存先（データは当面 identity の表）
/// から生徒を読むだけで、何も変えない。
///
/// 生徒のアカウントだけを返し（コーチ・本部は返さない）、スポーツを渡せばそのスポーツの生徒だけにする。
#[derive(Debug, Clone)]
pub struct ProfileAnnouncementRecipientLookup<R> {
    accounts: R,
}

impl<R> ProfileAnnouncementRecipientLookup<R> {
    pub fn new(accounts: R) -> Self {
        Self { accounts }
    }
}

impl<R> AnnouncementRecipientLookup for ProfileAnnouncementRecipientLookup<R>
where
    R: AccountRepository,
{
    async fn find_students(&self, sport: Option<Sport>) -> Result<Vec<AccountId>, Unavailable> {
        Ok(self
            .accounts
            .list_students()
            .await?
            .into_iter()
            .filter(|account| {
                account
                    .as_student()
                    .is_some_and(|student| sport.is_none_or(|sport| student.sport() == sport))
            })
            .map(|account| account.id())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        identity::{Account, Email, Role},
        profile::DisplayName,
    };

    use super::*;
    use crate::in_memory::InMemoryAccountRepository;

    fn account(id: u128, role: Role, sports: &[Sport]) -> Account {
        Account::new(
            AccountId::from_u128(id),
            Email::new(format!("user{id}@example.com")).unwrap(),
            DisplayName::new("テスト").unwrap(),
            role,
            sports.iter().copied(),
            SystemTime::UNIX_EPOCH + Duration::from_secs(id as u64),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn returns_students_of_the_sport_or_every_student() {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, Role::Student, &[Sport::Tennis]),
            account(2, Role::Student, &[Sport::Golf]),
            account(3, Role::Coach, &[Sport::Tennis]),
            account(4, Role::Headquarters, &[]),
            account(5, Role::Student, &[Sport::Tennis]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        let lookup = ProfileAnnouncementRecipientLookup::new(accounts);
        let ids = |students: Vec<AccountId>| {
            let mut ids: Vec<u128> = students.into_iter().map(AccountId::as_u128).collect();
            ids.sort();
            ids
        };

        assert_eq!(
            ids(lookup.find_students(None).await.unwrap()),
            [1, 2, 5],
            "生徒全員（ゴルフも含む）。コーチ・本部は含めない"
        );
        assert_eq!(
            ids(lookup.find_students(Some(Sport::Tennis)).await.unwrap()),
            [1, 5],
            "テニスの生徒だけ（テニスのコーチは含めない）"
        );
        assert!(
            lookup
                .find_students(Some(Sport::Swimming))
                .await
                .unwrap()
                .is_empty()
        );
    }
}
