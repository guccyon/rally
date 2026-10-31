use std::time::SystemTime;

use super::{AccountId, Coach, Email, Role, RoleAttributes, Student};
use crate::catalog::Sport;
use crate::{DomainError, profile::DisplayName};

/// アカウント。生徒・コーチ・本部のどれか。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    id: AccountId,
    email: Email,
    display_name: DisplayName,
    role_attributes: RoleAttributes,
    registered_at: SystemTime,
}

impl Account {
    /// 役割とスポーツの組み合わせを検証して作る。
    ///
    /// - 生徒: スポーツはちょうど 1 つ
    /// - コーチ: スポーツは 1 つ以上（重複は 1 つにまとめる）
    /// - 本部: スポーツを持たない
    pub fn new(
        id: AccountId,
        email: Email,
        display_name: DisplayName,
        role: Role,
        sports: impl IntoIterator<Item = Sport>,
        registered_at: SystemTime,
    ) -> Result<Self, DomainError> {
        let sports: Vec<Sport> = sports.into_iter().collect();
        let mismatch = || DomainError::RoleAttributeMismatch {
            role,
            sports: sports.len(),
        };
        let role_attributes = match role {
            Role::Student => match sports.as_slice() {
                [sport] => RoleAttributes::Student(Student::new(*sport)),
                _ => return Err(mismatch()),
            },
            Role::Coach => RoleAttributes::Coach(Coach::new(sports.iter().copied())?),
            Role::Headquarters if sports.is_empty() => RoleAttributes::Headquarters,
            Role::Headquarters => return Err(mismatch()),
        };
        Ok(Self::with_role_attributes(
            id,
            email,
            display_name,
            role_attributes,
            registered_at,
        ))
    }

    /// 検証済みの属性から作る（属性の型が組み合わせを保証する）。
    pub fn with_role_attributes(
        id: AccountId,
        email: Email,
        display_name: DisplayName,
        role_attributes: RoleAttributes,
        registered_at: SystemTime,
    ) -> Self {
        Self {
            id,
            email,
            display_name,
            role_attributes,
            registered_at,
        }
    }

    pub const fn id(&self) -> AccountId {
        self.id
    }

    pub const fn role(&self) -> Role {
        self.role_attributes.role()
    }

    pub fn email(&self) -> &Email {
        &self.email
    }

    pub fn display_name(&self) -> &DisplayName {
        &self.display_name
    }

    pub fn role_attributes(&self) -> &RoleAttributes {
        &self.role_attributes
    }

    pub const fn registered_at(&self) -> SystemTime {
        self.registered_at
    }

    /// 役割が生徒なら、その属性。
    pub fn as_student(&self) -> Option<&Student> {
        match &self.role_attributes {
            RoleAttributes::Student(student) => Some(student),
            _ => None,
        }
    }

    /// 役割がコーチなら、その属性。
    pub fn as_coach(&self) -> Option<&Coach> {
        match &self.role_attributes {
            RoleAttributes::Coach(coach) => Some(coach),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(role: Role, sports: &[Sport]) -> Result<Account, DomainError> {
        Account::new(
            AccountId::from_u128(1),
            Email::new("taro@example.com").unwrap(),
            DisplayName::new("たろう").unwrap(),
            role,
            sports.iter().copied(),
            SystemTime::UNIX_EPOCH,
        )
    }

    #[test]
    fn student_has_exactly_one_sport() {
        let student = account(Role::Student, &[Sport::Tennis]).unwrap();
        assert_eq!(student.role(), Role::Student);
        assert_eq!(student.as_student().unwrap().sport(), Sport::Tennis);
        assert!(student.as_coach().is_none());

        for sports in [&[][..], &[Sport::Tennis, Sport::Soccer][..]] {
            assert_eq!(
                account(Role::Student, sports),
                Err(DomainError::RoleAttributeMismatch {
                    role: Role::Student,
                    sports: sports.len()
                })
            );
        }
    }

    #[test]
    fn coach_offers_a_non_empty_set_of_sports() {
        let coach = account(Role::Coach, &[Sport::Golf, Sport::Tennis, Sport::Golf]).unwrap();
        assert_eq!(coach.role(), Role::Coach);
        let sports = coach.as_coach().unwrap().sports();
        assert_eq!(
            sports.iter().copied().collect::<Vec<_>>(),
            [Sport::Tennis, Sport::Golf]
        );
        assert!(coach.as_coach().unwrap().offers(Sport::Golf));
        assert!(!coach.as_coach().unwrap().offers(Sport::Soccer));

        assert_eq!(
            account(Role::Coach, &[]),
            Err(DomainError::RoleAttributeMismatch {
                role: Role::Coach,
                sports: 0
            })
        );
    }

    #[test]
    fn headquarters_has_no_sports() {
        let staff = account(Role::Headquarters, &[]).unwrap();
        assert_eq!(staff.role(), Role::Headquarters);
        assert!(staff.as_student().is_none() && staff.as_coach().is_none());

        assert_eq!(
            account(Role::Headquarters, &[Sport::Baseball]),
            Err(DomainError::RoleAttributeMismatch {
                role: Role::Headquarters,
                sports: 1
            })
        );
    }
}
