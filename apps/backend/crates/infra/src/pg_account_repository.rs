//! アカウントの保存先（Postgres の `identity.accounts`・`coaches`・`coach_sports`）。ID でのアカウントとコーチ・生徒の一覧も読み、
//! 生徒の表示名（`accounts.display_name`）を書き換える。profile のコーチの自己紹介文の保存先（[`CoachProfileRepository`]）も
//! 兼ね、当面は identity の表の `coaches.bio` を読み書きする（表を分けるのは後の段。docs/adr/0009-subdomain-types.md）。

use application::{
    Unavailable,
    identity::{AccountRepository, InsertAccountError, account_id_uuid},
    profile::CoachProfileRepository,
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

/// [`AccountRepository`] と [`CoachProfileRepository`] の Postgres 実装。
///
/// 生徒のスポーツは `accounts.sport`、コーチの提供スポーツは `coach_sports` に持つ。メールアドレスの一意性
/// （大文字小文字を区別しない）は DB の `accounts_email_key` で守る。
#[derive(Debug, Clone)]
pub struct PgAccountRepository {
    pool: PgPool,
}

/// コーチの列（account_id, email, display_name, created_at, 提供スポーツ）。
type CoachRow = (Uuid, String, String, OffsetDateTime, Vec<String>);

/// 生徒の列（account_id, email, display_name, created_at, 生徒のスポーツ）。
type StudentRow = (Uuid, String, String, OffsetDateTime, Option<String>);

/// アカウントの列（account_id, role, email, display_name, created_at, 生徒のスポーツ, コーチの提供スポーツ）。
type AccountRow = (
    Uuid,
    String,
    String,
    String,
    OffsetDateTime,
    Option<String>,
    Vec<String>,
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
        // 生徒のスポーツは accounts.sport、コーチの提供スポーツは coach_sports から集める
        let row: Option<AccountRow> = sqlx::query_as(
            r#"SELECT a.account_id, a.role, a.email, a.display_name, a.created_at, a.sport,
                      COALESCE(array_agg(s.sport) FILTER (WHERE s.sport IS NOT NULL), '{}')
               FROM identity.accounts a
               LEFT JOIN identity.coach_sports s ON s.coach_id = a.account_id
               WHERE a.account_id = $1
               GROUP BY a.account_id"#,
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
                      COALESCE(array_agg(s.sport) FILTER (WHERE s.sport IS NOT NULL), '{}')
               FROM identity.accounts a
               JOIN identity.coaches c ON c.coach_id = a.account_id
               LEFT JOIN identity.coach_sports s ON s.coach_id = a.account_id
               WHERE a.role = $1
               GROUP BY a.account_id
               ORDER BY a.display_name COLLATE "C", a.account_id"#,
        )
        .bind(role_value(Role::Coach))
        .fetch_all(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        rows.into_iter().map(coach_from_row).collect()
    }

    async fn list_students(&self) -> Result<Vec<Account>, Unavailable> {
        // 生徒は coach_sports・coaches の行を持たない（スポーツは accounts.sport）
        let rows: Vec<StudentRow> = sqlx::query_as(
            "SELECT account_id, email, display_name, created_at, sport
             FROM identity.accounts
             WHERE role = $1
             ORDER BY created_at DESC, account_id",
        )
        .bind(role_value(Role::Student))
        .fetch_all(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        rows.into_iter().map(student_from_row).collect()
    }

    async fn update_student_display_name(
        &self,
        student_id: AccountId,
        display_name: DisplayName,
    ) -> Result<Option<Account>, Unavailable> {
        let updated = sqlx::query(
            "UPDATE identity.accounts SET display_name = $2 WHERE account_id = $1 AND role = $3",
        )
        .bind(account_id_uuid::to_uuid(student_id))
        .bind(display_name.as_str())
        .bind(role_value(Role::Student))
        .execute(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        if updated.rows_affected() == 0 {
            return Ok(None);
        }
        self.find(student_id).await
    }
}

impl CoachProfileRepository for PgAccountRepository {
    async fn find_coach_bio(&self, coach_id: AccountId) -> Result<Option<CoachBio>, Unavailable> {
        // coaches に行があるのはコーチのアカウントだけ（coaches_account_fkey）
        let bio: Option<String> =
            sqlx::query_scalar("SELECT bio FROM identity.coaches WHERE coach_id = $1")
                .bind(account_id_uuid::to_uuid(coach_id))
                .fetch_optional(&self.pool)
                .await
                .map_err(Unavailable::new)?;
        bio.map(|bio| CoachBio::new(bio).map_err(Unavailable::new))
            .transpose()
    }

    async fn update_coach_bio(
        &self,
        coach_id: AccountId,
        bio: CoachBio,
    ) -> Result<Option<CoachBio>, Unavailable> {
        let updated = sqlx::query("UPDATE identity.coaches SET bio = $2 WHERE coach_id = $1")
            .bind(account_id_uuid::to_uuid(coach_id))
            .bind(bio.as_str())
            .execute(&self.pool)
            .await
            .map_err(Unavailable::new)?;
        if updated.rows_affected() == 0 {
            return Ok(None);
        }
        Ok(Some(bio))
    }
}

fn account_from_row(
    (account_id, role, email, display_name, created_at, student_sport, coach_sports): AccountRow,
) -> Result<Account, Unavailable> {
    let role =
        role_from_value(&role).ok_or_else(|| Unavailable::new(format!("未知の役割: {role}")))?;
    if role == Role::Coach {
        return coach_from_row((account_id, email, display_name, created_at, coach_sports));
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
    (account_id, email, display_name, created_at, sports): CoachRow,
) -> Result<Account, Unavailable> {
    let sports = sports_from_values(&sports)?;
    let coach = Coach::new(sports).map_err(Unavailable::new)?;
    Ok(Account::with_role_attributes(
        account_id_uuid::from_uuid(account_id),
        Email::new(email).map_err(Unavailable::new)?,
        DisplayName::new(display_name).map_err(Unavailable::new)?,
        RoleAttributes::Coach(coach),
        created_at.into(),
    ))
}

fn student_from_row(
    (account_id, email, display_name, created_at, sport): StudentRow,
) -> Result<Account, Unavailable> {
    Account::new(
        account_id_uuid::from_uuid(account_id),
        Email::new(email).map_err(Unavailable::new)?,
        DisplayName::new(display_name).map_err(Unavailable::new)?,
        Role::Student,
        sports_from_values(&sport)?,
        created_at.into(),
    )
    .map_err(Unavailable::new)
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
