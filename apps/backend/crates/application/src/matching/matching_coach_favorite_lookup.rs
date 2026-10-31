use domain::identity::AccountId;

use super::FavoriteCoachRepository;
use crate::{Unavailable, profile::CoachFavoriteLookup};

/// profile の [`CoachFavoriteLookup`] の matching による実装。お気に入りの保存先から読むだけで、何も変えない。
#[derive(Debug, Clone)]
pub struct MatchingCoachFavoriteLookup<F> {
    favorites: F,
}

impl<F> MatchingCoachFavoriteLookup<F> {
    pub fn new(favorites: F) -> Self {
        Self { favorites }
    }
}

impl<F> CoachFavoriteLookup for MatchingCoachFavoriteLookup<F>
where
    F: FavoriteCoachRepository,
{
    async fn is_favorite_coach(
        &self,
        student_id: AccountId,
        coach_id: AccountId,
    ) -> Result<bool, Unavailable> {
        Ok(self
            .favorites
            .find_by_student(student_id)
            .await?
            .iter()
            .any(|favorite| favorite.coach_id() == coach_id))
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::matching::FavoriteCoach;

    use super::*;
    use crate::in_memory::InMemoryFavoriteCoachRepository;

    #[tokio::test]
    async fn answers_whether_the_student_has_the_coach_in_favorites() {
        let (student, other, coach) = (
            AccountId::from_u128(1),
            AccountId::from_u128(2),
            AccountId::from_u128(3),
        );
        let lookup =
            MatchingCoachFavoriteLookup::new(InMemoryFavoriteCoachRepository::with_favorites([
                FavoriteCoach::restore(student, coach, SystemTime::UNIX_EPOCH),
            ]));
        assert!(lookup.is_favorite_coach(student, coach).await.unwrap());
        assert!(!lookup.is_favorite_coach(other, coach).await.unwrap());
        assert!(!lookup.is_favorite_coach(student, other).await.unwrap());
    }
}
