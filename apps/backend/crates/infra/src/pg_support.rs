//! Postgres の保存先の実装（`Pg*Repository`）が共有する変換とロック。

use std::time::SystemTime;

use domain::entitlement::YearMonth;
use sqlx::{Postgres, Transaction};
use time::{Date, Month, OffsetDateTime};

/// `timestamptz` の列に書く時刻。
pub(crate) fn timestamp(at: SystemTime) -> OffsetDateTime {
    OffsetDateTime::from(at)
}

/// 暦月を、その初日の `date`（`entitlement.live_lesson_quotas.month`）にする。
pub(crate) fn month_start(month: YearMonth) -> Date {
    let calendar_month = Month::try_from(month.month()).expect("YearMonth の月は 1〜12");
    Date::from_calendar_date(month.year(), calendar_month, 1).expect("暦月の初日は有効な日付")
}

/// `date` の暦月。
pub(crate) fn year_month(date: Date) -> YearMonth {
    YearMonth::new(date.year(), u8::from(date.month())).expect("time の月は 1〜12")
}

/// トランザクションが終わるまで `key` の advisory lock を取る（同じ集約への読み取りから保存までを直列にする）。
pub(crate) async fn lock(tx: &mut Transaction<'_, Postgres>, key: &str) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(key)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 一意制約 `constraint` の違反か。
pub(crate) fn is_unique_violation(error: &sqlx::Error, constraint: &str) -> bool {
    matches!(
        error,
        sqlx::Error::Database(e) if e.is_unique_violation() && e.constraint() == Some(constraint)
    )
}
