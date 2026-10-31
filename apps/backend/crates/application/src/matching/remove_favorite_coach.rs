use domain::identity::AccountId;

use super::FavoriteCoachRepository;
use crate::Unavailable;

/// コーチをお気に入りから外すユースケース（student-app の `removeFavoriteCoach`。生徒が呼ぶ）。
///
/// 入っていなければ何もしない（何度呼んでもよい）。コーチのアカウントは読まないので、コーチでなくなった・ない ID も
/// そのまま外せる。行動イベントも境界間イベントも発火しない。
pub struct RemoveFavoriteCoach<F> {
    favorites: F,
}

impl<F> RemoveFavoriteCoach<F>
where
    F: FavoriteCoachRepository,
{
    pub fn new(favorites: F) -> Self {
        Self { favorites }
    }

    /// 生徒 `student_id`（認証したアカウント。役割の確認は境界が行う）のお気に入りからコーチ `coach_id` を外す。
    pub async fn execute(
        &self,
        student_id: AccountId,
        coach_id: AccountId,
    ) -> Result<(), Unavailable> {
        self.favorites.remove(student_id, coach_id).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::matching::FavoriteCoach;

    use super::*;
    use crate::in_memory::InMemoryFavoriteCoachRepository;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(3);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);

    fn favorite(student_id: AccountId, coach_id: AccountId) -> FavoriteCoach {
        FavoriteCoach::restore(student_id, coach_id, SystemTime::UNIX_EPOCH)
    }

    #[tokio::test]
    async fn removes_only_the_coach_of_the_student_and_is_idempotent() {
        let favorites = InMemoryFavoriteCoachRepository::with_favorites([
            favorite(STUDENT, COACH),
            favorite(STUDENT, OTHER_COACH),
            favorite(OTHER_STUDENT, COACH),
        ]);
        let use_case = RemoveFavoriteCoach::new(favorites.clone());

        use_case.execute(STUDENT, COACH).await.unwrap();
        use_case.execute(STUDENT, COACH).await.unwrap();
        use_case
            .execute(STUDENT, AccountId::from_u128(99))
            .await
            .unwrap();

        assert_eq!(
            favorites.find_by_student(STUDENT).await.unwrap(),
            [favorite(STUDENT, OTHER_COACH)]
        );
        assert_eq!(
            favorites.find_by_student(OTHER_STUDENT).await.unwrap(),
            [favorite(OTHER_STUDENT, COACH)],
            "ほかの生徒のお気に入りは変えない"
        );
    }
}
