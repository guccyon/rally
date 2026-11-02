//! コーチの受付の保存先（Postgres の `scheduling.coach_acceptances` と、止めた・再開した記録の
//! `scheduling.coach_acceptance_changes`）。

use std::{collections::BTreeSet, time::SystemTime};

use application::{
    Unavailable,
    identity::account_id_uuid,
    scheduling::{CoachAcceptanceRepository, UpdateCoachAcceptanceError},
};
use domain::{
    identity::AccountId,
    scheduling::{
        CoachAcceptance, CoachAcceptanceChange, CoachAcceptanceStatus, CoachSuspensionReason,
        SchedulingError,
    },
};
use sqlx::{
    PgExecutor, PgPool,
    types::{Uuid, time::OffsetDateTime},
};

use crate::{
    pg_support::{lock, timestamp},
    scheduling_values::{coach_acceptance_status_from_value, coach_acceptance_status_value},
};

/// [`CoachAcceptanceRepository`] の Postgres 実装。
///
/// 一度も止めていないコーチは行を持たず、受付中として読む。止める・再開する規則は domain の [`CoachAcceptance`] だけで
/// 判定する。読み取りから保存までは 1 つのトランザクションで、コーチごとの advisory lock を取ってから行う（行がまだない
/// コーチへの並行な停止も直列にするため、行のロックではなく advisory lock にする）。止めた・再開した記録は同じ
/// トランザクションで `scheduling.coach_acceptance_changes` に 1 行足す（書き換えない）。止めた記録の理由は `reason` の列
/// （再開した記録と、列を足す前の記録は NULL）。
#[derive(Debug, Clone)]
pub struct PgCoachAcceptanceRepository {
    pool: PgPool,
}

impl PgCoachAcceptanceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// `coach_id` のコーチの受付を読む（行がなければ受付中）。予約の保存先（[`crate::PgBookingRepository`]）も使う。
pub(crate) async fn find_coach_acceptance<'e>(
    executor: impl PgExecutor<'e>,
    coach_id: AccountId,
) -> Result<CoachAcceptance, Unavailable> {
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM scheduling.coach_acceptances WHERE coach_id = $1")
            .bind(account_id_uuid::to_uuid(coach_id))
            .fetch_optional(executor)
            .await
            .map_err(Unavailable::new)?;
    let status = match status {
        Some(value) => coach_acceptance_status_from_value(&value)
            .ok_or_else(|| Unavailable::new(format!("未知の status: {value}")))?,
        None => CoachAcceptanceStatus::Accepting,
    };
    Ok(CoachAcceptance::restore(coach_id, status))
}

impl CoachAcceptanceRepository for PgCoachAcceptanceRepository {
    async fn update<F>(
        &self,
        coach_id: AccountId,
        update: F,
    ) -> Result<CoachAcceptance, UpdateCoachAcceptanceError>
    where
        F: FnOnce(&mut CoachAcceptance) -> Result<CoachAcceptanceChange, SchedulingError> + Send,
    {
        let unavailable = |e| UpdateCoachAcceptanceError::Unavailable(Unavailable::new(e));
        let coach = account_id_uuid::to_uuid(coach_id);
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        lock(&mut tx, &format!("scheduling.coach_acceptances:{coach}"))
            .await
            .map_err(unavailable)?;
        let mut acceptance = find_coach_acceptance(&mut *tx, coach_id)
            .await
            .map_err(UpdateCoachAcceptanceError::Unavailable)?;
        let change = update(&mut acceptance).map_err(UpdateCoachAcceptanceError::Rejected)?;
        sqlx::query(
            "INSERT INTO scheduling.coach_acceptances (coach_id, status, updated_at)
             VALUES ($1, $2, now())
             ON CONFLICT (coach_id) DO UPDATE SET status = EXCLUDED.status, updated_at = now()",
        )
        .bind(coach)
        .bind(coach_acceptance_status_value(acceptance.status()))
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
        sqlx::query(
            "INSERT INTO scheduling.coach_acceptance_changes
                 (coach_id, status, reason, changed_by, changed_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(account_id_uuid::to_uuid(change.coach_id()))
        .bind(coach_acceptance_status_value(change.status()))
        .bind(change.reason().map(CoachSuspensionReason::as_str))
        .bind(account_id_uuid::to_uuid(change.changed_by()))
        .bind(timestamp(change.changed_at()))
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(acceptance)
    }

    async fn suspended_coaches(&self) -> Result<BTreeSet<AccountId>, Unavailable> {
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT coach_id FROM scheduling.coach_acceptances WHERE status = $1",
        )
        .bind(coach_acceptance_status_value(
            CoachAcceptanceStatus::Suspended,
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        Ok(ids.into_iter().map(account_id_uuid::from_uuid).collect())
    }

    async fn changes(
        &self,
        coach_id: AccountId,
    ) -> Result<Vec<CoachAcceptanceChange>, Unavailable> {
        let rows: Vec<(String, Option<String>, Uuid, OffsetDateTime)> = sqlx::query_as(
            "SELECT status, reason, changed_by, changed_at FROM scheduling.coach_acceptance_changes
             WHERE coach_id = $1
             ORDER BY changed_at DESC, seq DESC",
        )
        .bind(account_id_uuid::to_uuid(coach_id))
        .fetch_all(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        rows.into_iter()
            .map(|(status, reason, changed_by, changed_at)| {
                let status = coach_acceptance_status_from_value(&status)
                    .ok_or_else(|| Unavailable::new(format!("未知の status: {status}")))?;
                let reason = reason
                    .map(CoachSuspensionReason::new)
                    .transpose()
                    .map_err(Unavailable::new)?;
                Ok(CoachAcceptanceChange::restore(
                    coach_id,
                    status,
                    reason,
                    account_id_uuid::from_uuid(changed_by),
                    SystemTime::from(changed_at),
                ))
            })
            .collect()
    }
}
