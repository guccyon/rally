//! 合成シードを書く: `cargo run -p seed -- --seed 42`（`apps/backend/README.md` の「合成シード」）。
//!
//! 環境変数 `DATABASE_URL` の Postgres にマイグレーションを当て、[`seed::SeedPlan`] を application の
//! ユースケースで書く。行動イベントの配信先は server と同じく `ANALYTICS_SINK` で選ぶ（[`infra::AnalyticsDestination`]）。

use std::{process::ExitCode, time::SystemTime};

use infra::AnalyticsDestination;
use seed::{SeedArgs, SeedPlan, SeedReport, Seeder};
use sqlx::PgPool;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let args = SeedArgs::parse(std::env::args().skip(1))
        .map_err(|e| format!("{e}\n{}", SeedArgs::USAGE))?;
    let url = std::env::var("DATABASE_URL")
        .map_err(|_| format!("DATABASE_URL が設定されていない\n{}", SeedArgs::USAGE))?;
    let analytics = AnalyticsDestination::from_env()?;
    let pool = PgPool::connect(&url)
        .await
        .map_err(|e| format!("DATABASE_URL に接続できない: {e}"))?;
    infra::MIGRATOR
        .run(&pool)
        .await
        .map_err(|e| format!("マイグレーションを当てられない: {e}"))?;

    let plan = SeedPlan::generate(args.seed, args.now.unwrap_or_else(SystemTime::now));
    Seeder::new(pool.clone(), analytics)
        .run(&plan)
        .await
        .map_err(|e| e.to_string())?;
    let report = SeedReport::load(&pool, plan.active_window_start())
        .await
        .map_err(|e| format!("書いた件数を読めない: {e}"))?;
    eprintln!(
        "seed {}: コーチ {}・生徒 {}・直近 30 日の活動 {}・上位 3 コーチの生徒の割合 {:.1}%",
        plan.seed,
        report.coaches,
        report.students,
        report.active_students,
        report.top_coach_share() * 100.0
    );
    eprintln!("{report:#?}");
    Ok(())
}
