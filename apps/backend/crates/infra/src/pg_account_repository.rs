//! アカウントの保存先（Postgres の `identity.accounts`・`coaches`・`coach_sports`）。

use application::{
    Unavailable,
    identity::{AccountRepository, InsertAccountError, account_id_uuid},
};
use domain::identity::{Account, AccountId, Profile};
use sqlx::{PgPool, types::Uuid};

use crate::{
    identity_values::{role_value, sport_value},
    pg_support::{is_unique_violation, timestamp},
};

/// [`AccountRepository`] の Postgres 実装。
///
/// 生徒のスポーツは `accounts.sport`、コーチの提供スポーツは `coach_sports` に持つ。メールアドレスの一意性
/// （大文字小文字を区別しない）は DB の `accounts_email_key` で守る。
#[derive(Debug, Clone)]
pub struct PgAccountRepository {
    pool: PgPool,
}

impl PgAccountRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl AccountRepository for PgAccountRepository {
    fn next_id(&self) -> AccountId {
        account_id_uuid::from_uuid(Uuid::new_v4())
    }

    async fn insert(&self, account: Account) -> Result<(), InsertAccountError> {
        let account_id = account_id_uuid::to_uuid(account.id());
        let registered_at = timestamp(account.registered_at());
        let student_sport = account.as_student().map(|s| sport_value(s.sport()));
        let mut tx = self.pool.begin().await.map_err(unavailable)?;

        sqlx::query(
            "INSERT INTO identity.accounts (account_id, role, email, display_name, sport, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(account_id)
        .bind(role_value(account.role()))
        .bind(account.email().as_str())
        .bind(account.display_name().as_str())
        .bind(student_sport)
        .bind(registered_at)
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            if is_unique_violation(&error, "accounts_email_key") {
                InsertAccountError::EmailAlreadyRegistered
            } else {
                unavailable(error)
            }
        })?;

        if let Profile::Coach(coach) = account.profile() {
            sqlx::query("INSERT INTO identity.coaches (coach_id, created_at) VALUES ($1, $2)")
                .bind(account_id)
                .bind(registered_at)
                .execute(&mut *tx)
                .await
                .map_err(unavailable)?;
            let sports: Vec<&str> = coach.sports().iter().copied().map(sport_value).collect();
            sqlx::query(
                "INSERT INTO identity.coach_sports (coach_id, sport) SELECT $1, unnest($2::text[])",
            )
            .bind(account_id)
            .bind(sports)
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        }
        tx.commit().await.map_err(unavailable)
    }
}

fn unavailable(error: sqlx::Error) -> InsertAccountError {
    InsertAccountError::Unavailable(Unavailable::new(error))
}
