//! 受講権と月額契約の保存先（Postgres の `entitlement.*`）。

use application::{
    Unavailable,
    entitlement::{
        EntitlementRepository, InsertEntitlementError, UpdateEntitlementError,
        entitlement_id_uuid, subscription_id_uuid,
    },
    identity::account_id_uuid,
    scheduling::booking_id_uuid,
};
use domain::{
    DomainError,
    entitlement::{
        ContractType, Entitlement, EntitlementId, LiveLessonQuota, Subscription, SubscriptionId,
    },
    identity::AccountId,
};
use sqlx::{
    PgPool, Postgres, Transaction,
    types::{Uuid, time::Date},
};

use crate::{
    entitlement_values::{contract_type_from_value, contract_type_value, subscription_status_value},
    identity_values::{sport_from_value, sport_value},
    pg_support::{lock, month_start, timestamp, year_month},
};

/// [`EntitlementRepository`] の Postgres 実装。
///
/// 月額の通話の枠は暦月ごとの `live_lesson_quotas` の行（残回数）と、その月に枠を使った予約の
/// `live_lesson_quota_bookings` の行で持ち（ADR 0005）、読むのは最後に記録した暦月の枠だけ。
/// 不変条件の判定は domain の [`Entitlement`] だけが行い、読み取りから保存までは生徒ごとの advisory lock で
/// 直列にする。
#[derive(Debug, Clone)]
pub struct PgEntitlementRepository {
    pool: PgPool,
}

/// `entitlement.entitlements` の 1 行（entitlement_id, contract_type, sport, subscription_id, remaining_video_reviews）。
type EntitlementRow = (Uuid, String, String, Option<Uuid>, Option<i32>);

impl PgEntitlementRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn lock_student(
        tx: &mut Transaction<'_, Postgres>,
        student_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        lock(tx, &format!("entitlement.entitlements:{student_id}")).await
    }

    async fn read(
        tx: &mut Transaction<'_, Postgres>,
        student_id: AccountId,
    ) -> Result<Option<Entitlement>, Unavailable> {
        let row: Option<EntitlementRow> = sqlx::query_as(
            "SELECT entitlement_id, contract_type, sport, subscription_id, remaining_video_reviews
             FROM entitlement.entitlements WHERE student_id = $1",
        )
        .bind(account_id_uuid::to_uuid(student_id))
        .fetch_optional(&mut **tx)
        .await
        .map_err(Unavailable::new)?;
        let Some((entitlement_id, contract_type, sport, subscription_id, remaining)) = row else {
            return Ok(None);
        };
        let id = entitlement_id_uuid::from_uuid(entitlement_id);
        let sport = sport_from_value(&sport)
            .ok_or_else(|| Unavailable::new(format!("未知の sport: {sport}")))?;
        let contract_type = contract_type_from_value(&contract_type)
            .ok_or_else(|| Unavailable::new(format!("未知の contract_type: {contract_type}")))?;
        let entitlement = match contract_type {
            ContractType::Monthly => {
                let subscription_id = subscription_id
                    .ok_or_else(|| Unavailable::new("月額の受講権に元の契約がない"))?;
                let quota = Self::read_quota(tx, entitlement_id).await?;
                Entitlement::monthly(
                    id,
                    student_id,
                    sport,
                    subscription_id_uuid::from_uuid(subscription_id),
                    quota,
                )
            }
            ContractType::OneShot => {
                let remaining = remaining
                    .and_then(|r| u32::try_from(r).ok())
                    .ok_or_else(|| Unavailable::new("ワンショットの受講権に残回数がない"))?;
                Entitlement::one_shot(id, student_id, sport, remaining)
            }
        };
        entitlement.map(Some).map_err(Unavailable::new)
    }

    /// 最後に記録した暦月の枠。
    async fn read_quota(
        tx: &mut Transaction<'_, Postgres>,
        entitlement_id: Uuid,
    ) -> Result<LiveLessonQuota, Unavailable> {
        let month: Option<Date> = sqlx::query_scalar(
            "SELECT max(month) FROM entitlement.live_lesson_quotas WHERE entitlement_id = $1",
        )
        .bind(entitlement_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(Unavailable::new)?;
        let month = month.ok_or_else(|| Unavailable::new("月額の受講権に通話の枠がない"))?;
        let bookings: Vec<Uuid> = sqlx::query_scalar(
            "SELECT booking_id FROM entitlement.live_lesson_quota_bookings
             WHERE entitlement_id = $1 AND month = $2",
        )
        .bind(entitlement_id)
        .bind(month)
        .fetch_all(&mut **tx)
        .await
        .map_err(Unavailable::new)?;
        LiveLessonQuota::restore(
            year_month(month),
            bookings.into_iter().map(booking_id_uuid::from_uuid),
        )
        .map_err(Unavailable::new)
    }

    /// 受講権の利用権（月額は通話の枠、ワンショットは残りの添削回数）を書く。
    async fn write_usage(
        tx: &mut Transaction<'_, Postgres>,
        entitlement: &Entitlement,
    ) -> Result<(), sqlx::Error> {
        let entitlement_id = entitlement_id_uuid::to_uuid(entitlement.id());
        if let Some(remaining) = entitlement.remaining_video_reviews() {
            sqlx::query(
                "UPDATE entitlement.entitlements SET remaining_video_reviews = $2
                 WHERE entitlement_id = $1",
            )
            .bind(entitlement_id)
            .bind(i32::try_from(remaining).unwrap_or(i32::MAX))
            .execute(&mut **tx)
            .await?;
        }
        let Some(quota) = entitlement.live_lesson_quota() else {
            return Ok(());
        };
        let month = month_start(quota.month());
        let bookings: Vec<Uuid> = quota.bookings().map(booking_id_uuid::to_uuid).collect();
        sqlx::query(
            "INSERT INTO entitlement.live_lesson_quotas (entitlement_id, month, remaining_live_lessons)
             VALUES ($1, $2, $3)
             ON CONFLICT (entitlement_id, month)
             DO UPDATE SET remaining_live_lessons = EXCLUDED.remaining_live_lessons",
        )
        .bind(entitlement_id)
        .bind(month)
        .bind(i16::from(quota.remaining()))
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "DELETE FROM entitlement.live_lesson_quota_bookings
             WHERE entitlement_id = $1 AND month = $2 AND NOT (booking_id = ANY($3))",
        )
        .bind(entitlement_id)
        .bind(month)
        .bind(&bookings)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "INSERT INTO entitlement.live_lesson_quota_bookings (entitlement_id, month, booking_id)
             SELECT $1, $2, unnest($3::uuid[])
             ON CONFLICT DO NOTHING",
        )
        .bind(entitlement_id)
        .bind(month)
        .bind(&bookings)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }
}

impl EntitlementRepository for PgEntitlementRepository {
    fn next_entitlement_id(&self) -> EntitlementId {
        entitlement_id_uuid::from_uuid(Uuid::new_v4())
    }

    fn next_subscription_id(&self) -> SubscriptionId {
        subscription_id_uuid::from_uuid(Uuid::new_v4())
    }

    async fn find_by_student(
        &self,
        student_id: AccountId,
    ) -> Result<Option<Entitlement>, Unavailable> {
        let mut tx = self.pool.begin().await.map_err(Unavailable::new)?;
        let entitlement = Self::read(&mut tx, student_id).await?;
        tx.commit().await.map_err(Unavailable::new)?;
        Ok(entitlement)
    }

    async fn insert_monthly(
        &self,
        subscription: Subscription,
        entitlement: Entitlement,
    ) -> Result<(), InsertEntitlementError> {
        let student_id = account_id_uuid::to_uuid(entitlement.student_id());
        let unavailable = |e: sqlx::Error| InsertEntitlementError::Unavailable(Unavailable::new(e));
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        Self::lock_student(&mut tx, student_id)
            .await
            .map_err(unavailable)?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM entitlement.entitlements WHERE student_id = $1)",
        )
        .bind(student_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(unavailable)?;
        if exists {
            return Err(InsertEntitlementError::AlreadyExists);
        }

        let started_at = timestamp(subscription.started_at());
        sqlx::query(
            "INSERT INTO entitlement.subscriptions (subscription_id, student_id, sport, status, started_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(subscription_id_uuid::to_uuid(subscription.id()))
        .bind(account_id_uuid::to_uuid(subscription.student_id()))
        .bind(sport_value(subscription.sport()))
        .bind(subscription_status_value(subscription.status()))
        .bind(started_at)
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
        sqlx::query(
            "INSERT INTO entitlement.entitlements
                 (entitlement_id, student_id, contract_type, sport, subscription_id,
                  unlimited_video_and_chat, remaining_video_reviews, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(entitlement_id_uuid::to_uuid(entitlement.id()))
        .bind(student_id)
        .bind(contract_type_value(entitlement.contract_type()))
        .bind(sport_value(entitlement.sport()))
        .bind(entitlement.subscription_id().map(subscription_id_uuid::to_uuid))
        .bind(entitlement.unlimited_video_and_chat())
        .bind(
            entitlement
                .remaining_video_reviews()
                .map(|r| i32::try_from(r).unwrap_or(i32::MAX)),
        )
        .bind(started_at)
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
        Self::write_usage(&mut tx, &entitlement)
            .await
            .map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)
    }

    async fn update_by_student<F>(
        &self,
        student_id: AccountId,
        update: F,
    ) -> Result<Entitlement, UpdateEntitlementError>
    where
        F: FnOnce(&mut Entitlement) -> Result<(), DomainError> + Send,
    {
        let unavailable = |e: sqlx::Error| UpdateEntitlementError::Unavailable(Unavailable::new(e));
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        Self::lock_student(&mut tx, account_id_uuid::to_uuid(student_id))
            .await
            .map_err(unavailable)?;
        let mut entitlement = Self::read(&mut tx, student_id)
            .await
            .map_err(UpdateEntitlementError::Unavailable)?
            .ok_or(UpdateEntitlementError::NotFound)?;
        update(&mut entitlement).map_err(UpdateEntitlementError::Rejected)?;
        Self::write_usage(&mut tx, &entitlement)
            .await
            .map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(entitlement)
    }
}
