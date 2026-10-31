use std::collections::BTreeSet;

use super::Role;
use crate::DomainError;
use crate::catalog::Sport;

/// コーチの値オブジェクト。提供スポーツの集合（空ではない）を持つ。
///
/// 自己紹介文は profile の持ち物で、ここには持たない（`profile::CoachBio`。読み書きは profile のポート）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coach {
    sports: BTreeSet<Sport>,
}

impl Coach {
    /// 提供スポーツから作る。
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

    #[test]
    fn coach_offers_only_its_sports() {
        let coach = Coach::new([Sport::Tennis, Sport::Golf, Sport::Tennis]).unwrap();
        assert_eq!(
            coach.sports(),
            &BTreeSet::from([Sport::Golf, Sport::Tennis])
        );
        assert!(coach.offers(Sport::Golf));
        assert!(!coach.offers(Sport::Soccer));
    }
}
