use crate::{catalog::Sport, identity::Account};

/// 本部がコーチを競技で絞るときの条件（admin-web の `listCoaches` と `listIncompleteCoachProfiles` の `sport`）。
/// 「コーチの競技」をどう決めるかはここだけに置く。
///
/// - 競技 `Some(sport)`: コーチの役割のアカウントで、提供スポーツのどれかが `sport` と一致するものだけ（ゴルフも絞れる）
/// - `None`（既定）: 絞らない（どのアカウントも通す）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CoachSportFilter(Option<Sport>);

impl CoachSportFilter {
    /// 競技 `sport` で絞る条件。`None` なら絞らない。
    pub const fn new(sport: Option<Sport>) -> Self {
        Self(sport)
    }

    /// 絞る競技。絞らないなら `None`。
    pub const fn sport(&self) -> Option<Sport> {
        self.0
    }

    /// `account` がこの条件に合うか。
    pub fn matches(&self, account: &Account) -> bool {
        self.0
            .is_none_or(|sport| account.as_coach().is_some_and(|coach| coach.offers(sport)))
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;
    use crate::{
        identity::{AccountId, Email, Role},
        profile::DisplayName,
    };

    fn account(role: Role, sports: &[Sport]) -> Account {
        Account::new(
            AccountId::from_u128(1),
            Email::new("taro@example.com").unwrap(),
            DisplayName::new("たろう").unwrap(),
            role,
            sports.iter().copied(),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    #[test]
    fn matches_coach_offering_the_sport_among_several() {
        let coach = account(Role::Coach, &[Sport::Tennis, Sport::Golf]);
        assert!(CoachSportFilter::new(Some(Sport::Tennis)).matches(&coach));
        assert!(CoachSportFilter::new(Some(Sport::Golf)).matches(&coach));
        assert!(!CoachSportFilter::new(Some(Sport::Soccer)).matches(&coach));
    }

    #[test]
    fn sport_never_matches_students_or_headquarters() {
        let student = account(Role::Student, &[Sport::Tennis]);
        let headquarters = account(Role::Headquarters, &[]);
        let filter = CoachSportFilter::new(Some(Sport::Tennis));
        assert!(
            !filter.matches(&student),
            "生徒のスポーツはコーチの競技ではない"
        );
        assert!(!filter.matches(&headquarters));
    }

    #[test]
    fn no_sport_matches_everyone() {
        let filter = CoachSportFilter::default();
        assert_eq!(filter, CoachSportFilter::new(None));
        assert_eq!(filter.sport(), None);
        for account in [
            account(Role::Coach, &[Sport::Baseball]),
            account(Role::Student, &[Sport::Tennis]),
            account(Role::Headquarters, &[]),
        ] {
            assert!(filter.matches(&account));
        }
    }
}
