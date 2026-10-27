//! 合成シード（T18）のデータで、スポーツごとの予約枠の需給（admin-web の `getSlotSupply`）に「足りていない」
//! スポーツが少なくとも 1 つあること（Issue #77 の受け入れ条件）。シードを種 42 と固定した時刻で Postgres に書き、
//! 同じ時刻で `GetSlotSupply` を Postgres の保存先に通す。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p seed -- --include-ignored` で走らせる（テストごとに新しい DB を作ってマイグレーションを当てる）。

use std::time::{Duration, SystemTime};

use application::{
    in_memory::{FixedClock, RecordingAnalyticsSink},
    scheduling::GetSlotSupply,
};
use domain::{
    identity::Sport,
    scheduling::{SlotSupply, SlotSupplyStatus},
};
use infra::PgBookingRepository;
use seed::{SeedPlan, Seeder};
use sqlx::PgPool;

/// 2026-10-26T12:00:00+09:00（シードの計画のテストと同じ時刻）
const NOW: Duration = Duration::from_secs(1_792_983_600);

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn some_sport_lacks_slots_in_the_synthetic_seed(pool: PgPool) {
    let now = SystemTime::UNIX_EPOCH + NOW;
    let plan = SeedPlan::generate(42, now);
    Seeder::new(pool.clone(), RecordingAnalyticsSink::default())
        .run(&plan)
        .await
        .unwrap();

    let report = GetSlotSupply::new(PgBookingRepository::new(pool), FixedClock(now))
        .execute()
        .await
        .unwrap();
    let insufficient: Vec<&SlotSupply> = report
        .sports
        .iter()
        .filter(|s| s.status() == SlotSupplyStatus::Insufficient)
        .collect();
    assert!(!insufficient.is_empty(), "{:?}", report.sports);
    // すべてが足りていないのでは採用の優先が決まらない。足りているスポーツもある
    assert!(
        insufficient.len() < SeedPlan::SPORTS.len(),
        "{:?}",
        report.sports
    );
    // シードはゴルフのコーチを作らない
    let golf = report.sports.iter().find(|s| s.sport() == Sport::Golf);
    assert_eq!(golf, Some(&SlotSupply::new(Sport::Golf, 0, 0)));
}
