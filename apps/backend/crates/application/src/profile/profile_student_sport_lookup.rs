use domain::{catalog::Sport, identity::AccountId};

use crate::{Unavailable, identity::AccountRepository, matching::StudentSportLookup};

/// matching の [`StudentSportLookup`] の profile による実装。アカウントの保存先（データは当面 identity の表）から
/// 読むだけで、何も変えない。
///
/// 生徒でないアカウントと、ないアカウントは `None`。
#[derive(Debug, Clone)]
pub struct ProfileStudentSportLookup<R> {
    accounts: R,
}

impl<R> ProfileStudentSportLookup<R> {
    pub fn new(accounts: R) -> Self {
        Self { accounts }
    }
}

impl<R> StudentSportLookup for ProfileStudentSportLookup<R>
where
    R: AccountRepository,
{
    async fn find_student_sport(
        &self,
        student_id: AccountId,
    ) -> Result<Option<Sport>, Unavailable> {
        Ok(self
            .accounts
            .find(student_id)
            .await?
            .and_then(|account| account.as_student().map(|student| student.sport())))
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

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
            SystemTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn returns_the_sport_of_students_only() {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, Role::Student, &[Sport::Swimming]),
            account(2, Role::Coach, &[Sport::Tennis]),
        ] {
            accounts.insert(account).await.unwrap();
        }

        let lookup = ProfileStudentSportLookup::new(accounts);
        let sport_of = async |id| {
            lookup
                .find_student_sport(AccountId::from_u128(id))
                .await
                .unwrap()
        };
        assert_eq!(sport_of(1).await, Some(Sport::Swimming));
        assert_eq!(sport_of(2).await, None, "コーチは生徒でない");
        assert_eq!(sport_of(3).await, None, "ないアカウント");
    }
}
