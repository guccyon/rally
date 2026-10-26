use std::collections::BTreeSet;

use super::{Role, Sport};
use crate::DomainError;

/// コーチの値オブジェクト。提供スポーツの集合を持ち、空ではない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coach {
    sports: BTreeSet<Sport>,
}

impl Coach {
    pub fn new(sports: impl IntoIterator<Item = Sport>) -> Result<Self, DomainError> {
        let sports: BTreeSet<Sport> = sports.into_iter().collect();
        if sports.is_empty() {
            return Err(DomainError::RoleAttributeMismatch {
                role: Role::Coach,
                sports: 0,
            });
        }
        Ok(Self { sports })
    }

    pub fn sports(&self) -> &BTreeSet<Sport> {
        &self.sports
    }

    pub fn offers(&self, sport: Sport) -> bool {
        self.sports.contains(&sport)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coach_without_sports_is_a_domain_error() {
        assert_eq!(
            Coach::new([]),
            Err(DomainError::RoleAttributeMismatch {
                role: Role::Coach,
                sports: 0
            })
        );
    }
}
