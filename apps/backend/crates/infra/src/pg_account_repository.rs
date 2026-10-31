//! アカウントの保存先（Postgres の `identity.accounts`・`coaches`・`coach_sports`）。ID でのアカウントとコーチの一覧も読み、
//! コーチの自己紹介文（`coaches.bio`）を書き換える。

use application::{
    Unavailable,
    identity::{AccountRepository, InsertAccountError, account_id_uuid},
};
use domain::{
    catalog::Sport,
    identity::{Account, AccountId, Coach, Email, Role, RoleAttributes},
    profile::{CoachBio, DisplayName},
};
use sqlx::{
    PgPool,
    types::{Uuid, time::OffsetDateTime},
};

use crate::{
    identity_values::{role_from_value, role_value, sport_from_value, sport_value},
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

/// コーチの列（account_id, email, display_name, created_at, 提供スポーツ, 自己紹介文）。
type CoachRow = (Uuid, String, String, OffsetDateTime, Vec<String>, String);

/// アカウントの列（account_id, role, email, display_name, created_at, 生徒のスポーツ, コーチの提供スポーツ,
/// コーチの自己紹介文）。
type AccountRow = (
    Uuid,
    String,
    String,
    String,
    OffsetDateTime,
    Option<String>,
    Vec<String>,
    Option<String>,
);

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

        if let RoleAttributes::Coach(coach) = account.role_attributes() {
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

    async fn find(&self, id: AccountId) -> Result<Option<Account>, Unavailable> {
        // 生徒のスポーツは accounts.sport、コーチの提供スポーツは coach_sports、自己紹介文は coaches から集める
        let row: Option<AccountRow> = sqlx::query_as(
            r#"SELECT a.account_id, a.role, a.email, a.display_name, a.created_at, a.sport,
                      COALESCE(array_agg(s.sport) FILTER (WHERE s.sport IS NOT NULL), '{}'),
                      c.bio
               FROM identity.accounts a
               LEFT JOIN identity.coaches c ON c.coach_id = a.account_id
               LEFT JOIN identity.coach_sports s ON s.coach_id = a.account_id
               WHERE a.account_id = $1
               GROUP BY a.account_id, c.coach_id"#,
        )
        .bind(account_id_uuid::to_uuid(id))
        .fetch_optional(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        row.map(account_from_row).transpose()
    }

    async fn list_coaches(&self) -> Result<Vec<Account>, Unavailable> {
        // 表示名は照合順序に依らずコードポイントの順（COLLATE "C"。UTF-8 のバイト順と同じ）で並べる
        let rows: Vec<CoachRow> = sqlx::query_as(
            r#"SELECT a.account_id, a.email, a.display_name, a.created_at,
                      COALESCE(array_agg(s.sport) FILTER (WHERE s.sport IS NOT NULL), '{}'),
                      c.bio
               FROM identity.accounts a
               JOIN identity.coaches c ON c.coach_id = a.account_id
               LEFT JOIN identity.coach_sports s ON s.coach_id = a.account_id
               WHERE a.role = $1
               GROUP BY a.account_id, c.coach_id
               ORDER BY a.display_name COLLATE "C", a.account_id"#,
        )
        .bind(role_value(Role::Coach))
        .fetch_all(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        rows.into_iter().map(coach_from_row).collect()
    }

    async fn update_coach_bio(
        &self,
        coach_id: AccountId,
        bio: CoachBio,
    ) -> Result<Option<Account>, Unavailable> {
        // coaches に行があるのはコーチのアカウントだけ（coaches_account_fkey）
        let updated = sqlx::query("UPDATE identity.coaches SET bio = $2 WHERE coach_id = $1")
            .bind(account_id_uuid::to_uuid(coach_id))
            .bind(bio.as_str())
            .execute(&self.pool)
            .await
            .map_err(Unavailable::new)?;
        if updated.rows_affected() == 0 {
            return Ok(None);
        }
        self.find(coach_id).await
    }
}

fn account_from_row(
    (account_id, role, email, display_name, created_at, student_sport, coach_sports, coach_bio): AccountRow,
) -> Result<Account, Unavailable> {
    let role =
        role_from_value(&role).ok_or_else(|| Unavailable::new(format!("未知の役割: {role}")))?;
    if role == Role::Coach {
        let bio = coach_bio
            .ok_or_else(|| Unavailable::new("コーチのアカウントに identity.coaches の行がない"))?;
        return coach_from_row((
            account_id,
            email,
            display_name,
            created_at,
            coach_sports,
            bio,
        ));
    }
    let sports = sports_from_values(student_sport.iter().chain(&coach_sports))?;
    Account::new(
        account_id_uuid::from_uuid(account_id),
        Email::new(email).map_err(Unavailable::new)?,
        DisplayName::new(display_name).map_err(Unavailable::new)?,
        role,
        sports,
        created_at.into(),
    )
    .map_err(Unavailable::new)
}

fn coach_from_row(
    (account_id, email, display_name, created_at, sports, bio): CoachRow,
) -> Result<Account, Unavailable> {
    let sports = sports_from_values(&sports)?;
    let coach = Coach::new(sports)
        .map_err(Unavailable::new)?
        .with_bio(CoachBio::new(bio).map_err(Unavailable::new)?);
    Ok(Account::with_role_attributes(
        account_id_uuid::from_uuid(account_id),
        Email::new(email).map_err(Unavailable::new)?,
        DisplayName::new(display_name).map_err(Unavailable::new)?,
        RoleAttributes::Coach(coach),
        created_at.into(),
    ))
}

fn sports_from_values<'a>(
    values: impl IntoIterator<Item = &'a String>,
) -> Result<Vec<Sport>, Unavailable> {
    values
        .into_iter()
        .map(|value| {
            sport_from_value(value)
                .ok_or_else(|| Unavailable::new(format!("未知のスポーツ: {value}")))
        })
        .collect()
}

fn unavailable(error: sqlx::Error) -> InsertAccountError {
    InsertAccountError::Unavailable(Unavailable::new(error))
}
