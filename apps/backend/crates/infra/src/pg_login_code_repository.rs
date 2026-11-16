//! ログインの確認コードの保存先（Postgres の `identity.login_codes`）。確認コードはアカウントの identity の表と同じ
//! スキーマに置くので、[`PgAccountRepository`] が [`LoginCodeRepository`] も実装する。

use application::{
    Unavailable,
    identity::{LoginCodeRepository, account_id_uuid},
};
use domain::identity::{Email, LoginCode, LoginCodeChallenge};
use sqlx::types::{Uuid, time::OffsetDateTime};

use crate::{pg_account_repository::PgAccountRepository, pg_support::timestamp};

/// `identity.login_codes` の 1 行（email, account_id, code, issued_at, failed_attempts）。
type LoginCodeRow = (String, Option<Uuid>, String, OffsetDateTime, i32);

impl LoginCodeRepository for PgAccountRepository {
    async fn find_login_code(
        &self,
        email: &Email,
    ) -> Result<Option<LoginCodeChallenge>, Unavailable> {
        let row: Option<LoginCodeRow> = sqlx::query_as(
            "SELECT email, account_id, code, issued_at, failed_attempts
             FROM identity.login_codes WHERE email_key = $1",
        )
        .bind(email.normalized())
        .fetch_optional(self.pool())
        .await
        .map_err(Unavailable::new)?;
        row.map(login_code_from_row).transpose()
    }

    async fn save_login_code(&self, challenge: LoginCodeChallenge) -> Result<(), Unavailable> {
        sqlx::query(
            "INSERT INTO identity.login_codes (email_key, email, account_id, code, issued_at, failed_attempts)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (email_key) DO UPDATE SET
                 email = EXCLUDED.email,
                 account_id = EXCLUDED.account_id,
                 code = EXCLUDED.code,
                 issued_at = EXCLUDED.issued_at,
                 failed_attempts = EXCLUDED.failed_attempts",
        )
        .bind(challenge.email().normalized())
        .bind(challenge.email().as_str())
        .bind(challenge.account_id().map(account_id_uuid::to_uuid))
        .bind(challenge.code().as_str())
        .bind(timestamp(challenge.issued_at()))
        .bind(i32::try_from(challenge.failed_attempts()).unwrap_or(i32::MAX))
        .execute(self.pool())
        .await
        .map_err(Unavailable::new)?;
        Ok(())
    }

    async fn record_failed_login_code_attempt(
        &self,
        challenge: &LoginCodeChallenge,
    ) -> Result<(), Unavailable> {
        // 読んだ値に足して書くのではなく DB の値に足す（同時の間違いを数え漏らさない）
        sqlx::query(
            "UPDATE identity.login_codes SET failed_attempts = failed_attempts + 1
             WHERE email_key = $1 AND code = $2 AND issued_at = $3",
        )
        .bind(challenge.email().normalized())
        .bind(challenge.code().as_str())
        .bind(timestamp(challenge.issued_at()))
        .execute(self.pool())
        .await
        .map_err(Unavailable::new)?;
        Ok(())
    }

    async fn consume_login_code(
        &self,
        challenge: &LoginCodeChallenge,
    ) -> Result<bool, Unavailable> {
        let deleted = sqlx::query(
            "DELETE FROM identity.login_codes
             WHERE email_key = $1 AND code = $2 AND issued_at = $3",
        )
        .bind(challenge.email().normalized())
        .bind(challenge.code().as_str())
        .bind(timestamp(challenge.issued_at()))
        .execute(self.pool())
        .await
        .map_err(Unavailable::new)?;
        Ok(deleted.rows_affected() == 1)
    }
}

fn login_code_from_row(
    (email, account_id, code, issued_at, failed_attempts): LoginCodeRow,
) -> Result<LoginCodeChallenge, Unavailable> {
    Ok(LoginCodeChallenge::restore(
        Email::new(email).map_err(Unavailable::new)?,
        account_id.map(account_id_uuid::from_uuid),
        LoginCode::new(code).map_err(Unavailable::new)?,
        issued_at.into(),
        u32::try_from(failed_attempts).map_err(Unavailable::new)?,
    ))
}
