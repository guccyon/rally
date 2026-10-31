//! お気に入りのコーチの保存先（Postgres の `matching.favorite_coaches`）。

use std::time::SystemTime;

use application::{Unavailable, identity::account_id_uuid, matching::FavoriteCoachRepository};
use domain::{identity::AccountId, matching::FavoriteCoach};
use sqlx::{
    PgPool,
    types::{Uuid, time::OffsetDateTime},
};

use crate::pg_support::timestamp;

/// [`FavoriteCoachRepository`] の Postgres 実装。
///
/// 生徒とコーチの組で 1 行（`favorite_coaches_pkey`）。入れ直しは `ON CONFLICT DO NOTHING` で、最初に入れた時刻のまま。
/// 生徒・コーチの役割は DB でも役割つきの外部キーで守る（判定は domain）。
#[derive(Debug, Clone)]
pub struct PgFavoriteCoachRepository {
    pool: PgPool,
}

impl PgFavoriteCoachRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl FavoriteCoachRepository for PgFavoriteCoachRepository {
    async fn add(&self, favorite: FavoriteCoach) -> Result<(), Unavailable> {
        sqlx::query(
            "INSERT INTO matching.favorite_coaches (student_id, coach_id, favorited_at)
             VALUES ($1, $2, $3)
             ON CONFLICT ON CONSTRAINT favorite_coaches_pkey DO NOTHING",
        )
        .bind(account_id_uuid::to_uuid(favorite.student_id()))
        .bind(account_id_uuid::to_uuid(favorite.coach_id()))
        .bind(timestamp(favorite.favorited_at()))
        .execute(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        Ok(())
    }

    async fn remove(&self, student_id: AccountId, coach_id: AccountId) -> Result<(), Unavailable> {
        sqlx::query(
            "DELETE FROM matching.favorite_coaches WHERE student_id = $1 AND coach_id = $2",
        )
        .bind(account_id_uuid::to_uuid(student_id))
        .bind(account_id_uuid::to_uuid(coach_id))
        .execute(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        Ok(())
    }

    async fn find_by_student(
        &self,
        student_id: AccountId,
    ) -> Result<Vec<FavoriteCoach>, Unavailable> {
        let rows: Vec<(Uuid, OffsetDateTime)> = sqlx::query_as(
            "SELECT coach_id, favorited_at FROM matching.favorite_coaches
             WHERE student_id = $1
             ORDER BY favorited_at DESC, coach_id",
        )
        .bind(account_id_uuid::to_uuid(student_id))
        .fetch_all(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        Ok(rows
            .into_iter()
            .map(|(coach_id, favorited_at)| {
                FavoriteCoach::restore(
                    student_id,
                    account_id_uuid::from_uuid(coach_id),
                    SystemTime::from(favorited_at),
                )
            })
            .collect())
    }
}
