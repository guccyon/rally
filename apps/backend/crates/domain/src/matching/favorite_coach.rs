use std::time::SystemTime;

use super::MatchingError;
use crate::identity::{Account, AccountId};

/// お気に入りのコーチ（glossary の favorite_coach）。生徒が、また開くために残しておいたコーチ 1 人。
///
/// 生徒と、コーチと、入れた時刻を持つ。同じ生徒と同じコーチの組は 1 つで、入れ直しても最初に入れた時刻のまま
/// （保存先が組で守る）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FavoriteCoach {
    student_id: AccountId,
    coach_id: AccountId,
    favorited_at: SystemTime,
}

impl FavoriteCoach {
    /// 生徒 `student` が時刻 `favorited_at` にコーチ `coach` をお気に入りに入れる。`student` が生徒でなければ
    /// [`MatchingError::NotAStudent`]、`coach` がコーチでなければ [`MatchingError::NotACoach`]。
    pub fn new(
        student: &Account,
        coach: &Account,
        favorited_at: SystemTime,
    ) -> Result<Self, MatchingError> {
        if student.as_student().is_none() {
            return Err(MatchingError::NotAStudent);
        }
        if coach.as_coach().is_none() {
            return Err(MatchingError::NotACoach);
        }
        Ok(Self {
            student_id: student.id(),
            coach_id: coach.id(),
            favorited_at,
        })
    }

    /// 保存先から読んだお気に入りを組み立てる（保存先の実装用。役割は保存先が守る）。
    pub fn restore(student_id: AccountId, coach_id: AccountId, favorited_at: SystemTime) -> Self {
        Self {
            student_id,
            coach_id,
            favorited_at,
        }
    }

    pub fn student_id(&self) -> AccountId {
        self.student_id
    }

    pub fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    /// お気に入りに入れた時刻（入れ直しても最初の時刻）。
    pub fn favorited_at(&self) -> SystemTime {
        self.favorited_at
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        catalog::Sport,
        identity::{Email, Role},
        profile::DisplayName,
    };

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

    #[test]
    fn a_student_favorites_a_coach() {
        let student = account(1, Role::Student, &[Sport::Tennis]);
        let coach = account(2, Role::Coach, &[Sport::Soccer]);
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(60);

        let favorite = FavoriteCoach::new(&student, &coach, at).unwrap();
        assert_eq!(favorite.student_id(), AccountId::from_u128(1));
        assert_eq!(favorite.coach_id(), AccountId::from_u128(2));
        assert_eq!(favorite.favorited_at(), at);
        assert_eq!(
            favorite,
            FavoriteCoach::restore(AccountId::from_u128(1), AccountId::from_u128(2), at),
            "スポーツが違っても入れられる"
        );
    }

    #[test]
    fn only_a_student_can_favorite() {
        let coach = account(2, Role::Coach, &[Sport::Tennis]);
        for not_a_student in [
            account(3, Role::Coach, &[Sport::Tennis]),
            account(4, Role::Headquarters, &[]),
        ] {
            assert_eq!(
                FavoriteCoach::new(&not_a_student, &coach, SystemTime::UNIX_EPOCH),
                Err(MatchingError::NotAStudent),
                "{:?}",
                not_a_student.role()
            );
        }
    }

    #[test]
    fn only_a_coach_can_be_favorited() {
        let student = account(1, Role::Student, &[Sport::Tennis]);
        for not_a_coach in [
            account(5, Role::Student, &[Sport::Tennis]),
            account(4, Role::Headquarters, &[]),
        ] {
            assert_eq!(
                FavoriteCoach::new(&student, &not_a_coach, SystemTime::UNIX_EPOCH),
                Err(MatchingError::NotACoach),
                "{:?}",
                not_a_coach.role()
            );
        }
        assert_eq!(
            FavoriteCoach::new(&student, &student, SystemTime::UNIX_EPOCH),
            Err(MatchingError::NotACoach),
            "自分自身は入れられない"
        );
    }
}
