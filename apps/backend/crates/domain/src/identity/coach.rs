use std::collections::BTreeSet;

use super::{Role, Sport};
use crate::{DomainError, profile::CoachBio};

/// コーチの値オブジェクト。提供スポーツの集合（空ではない）と自己紹介文を持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coach {
    sports: BTreeSet<Sport>,
    bio: CoachBio,
}

impl Coach {
    /// 提供スポーツから作る。自己紹介文は空。
    pub fn new(sports: impl IntoIterator<Item = Sport>) -> Result<Self, DomainError> {
        let sports: BTreeSet<Sport> = sports.into_iter().collect();
        if sports.is_empty() {
            return Err(DomainError::RoleAttributeMismatch {
                role: Role::Coach,
                sports: 0,
            });
        }
        Ok(Self {
            sports,
            bio: CoachBio::default(),
        })
    }

    /// 自己紹介文を `bio` にしたコーチ（提供スポーツはそのまま）。
    pub fn with_bio(self, bio: CoachBio) -> Self {
        Self { bio, ..self }
    }

    pub fn sports(&self) -> &BTreeSet<Sport> {
        &self.sports
    }

    pub fn offers(&self, sport: Sport) -> bool {
        self.sports.contains(&sport)
    }

    pub fn bio(&self) -> &CoachBio {
        &self.bio
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
    fn new_coach_has_an_empty_bio_and_with_bio_keeps_the_sports() {
        let coach = Coach::new([Sport::Tennis, Sport::Golf]).unwrap();
        assert_eq!(coach.bio(), &CoachBio::default());

        let bio = CoachBio::new("テニス歴 20 年").unwrap();
        let updated = coach.clone().with_bio(bio.clone());
        assert_eq!(updated.bio(), &bio);
        assert_eq!(updated.sports(), coach.sports());
    }
}
