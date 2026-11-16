//! 合成シード（種 42）のデータで、事業の KPI（admin-web の `getBusinessKpis`）の 6 指標の今の期間と前の期間に値が出ること
//! （Issue #1495 の受け入れ条件「staging の合成シードで値が出る」）。シードを種 42 と固定した時刻で Postgres に書き、
//! 同じ時刻で `GetBusinessKpis` を Postgres の業務の表（`PgKpiEventSource`）と kpi.yaml の正本（`FileKpiSpec`）に通す。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p seed -- --include-ignored` で走らせる（テストごとに新しい DB を作ってマイグレーションを当てる）。

use std::time::{Duration, SystemTime};

use application::{
    analytics::GetBusinessKpis,
    in_memory::{FixedClock, RecordingAnalyticsSink},
};
use infra::{FileKpiSpec, PgKpiEventSource};
use seed::{SeedPlan, Seeder};
use sqlx::PgPool;

/// 2026-10-26T12:00:00+09:00（シードの計画のテストと同じ時刻）
const NOW: Duration = Duration::from_secs(1_792_983_600);

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn six_kpis_have_values_in_the_synthetic_seed(pool: PgPool) {
    let now = SystemTime::UNIX_EPOCH + NOW;
    let plan = SeedPlan::generate(42, now);
    Seeder::new(pool.clone(), RecordingAnalyticsSink::default())
        .run(&plan)
        .await
        .unwrap();

    let kpis = GetBusinessKpis::new(
        FileKpiSpec::new(FileKpiSpec::DEFAULT_PATH),
        PgKpiEventSource::new(pool),
        FixedClock(now),
    )
    .execute()
    .await
    .unwrap();

    assert_eq!(kpis.kpis.len(), 6);
    for kpi in &kpis.kpis {
        assert!(
            kpi.current.value.is_ok(),
            "{} の今の期間: {:?}",
            kpi.id,
            kpi.current.value
        );
        assert!(
            kpi.previous.value.is_ok(),
            "{} の前の期間: {:?}",
            kpi.id,
            kpi.previous.value
        );
    }
}
