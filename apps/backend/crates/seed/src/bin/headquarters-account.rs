//! 本部のアカウントを作る: `headquarters-account --email <メールアドレス> --display-name <表示名>`
//! （`apps/backend/README.md` の「本部のアカウントを作る」。本番の空の DB から事業を始めるときの最初の 1 人）。
//!
//! 環境変数 `DATABASE_URL` の Postgres にマイグレーションを当て、登録のユースケース（`RegisterAccount`。`Role::Headquarters`）
//! で 1 つ登録して、アカウント ID を標準出力に書く。同じメールアドレスの本部のアカウントがあれば何もせず、その ID を書く
//! （[`seed::HeadquartersAccount::ensure`]。何度実行してもよい）。行動イベントの配信先は seed と同じく `ANALYTICS_SINK`
//! で選ぶ（本部の登録で発火するイベントはない）。トークンは発行しない（作ったあとは `access-token --account <ID>`）。

use std::process::ExitCode;

use application::{
    clock::SystemClock,
    identity::{RegisterAccount, account_id_uuid},
};
use infra::{AnalyticsDestination, PgAccountRepository};
use seed::{HeadquartersAccount, HeadquartersAccountArgs};
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
    let args = HeadquartersAccountArgs::parse(std::env::args().skip(1))
        .map_err(|e| format!("{e}\n{}", HeadquartersAccountArgs::USAGE))?;
    let url = std::env::var("DATABASE_URL").map_err(|_| {
        format!(
            "DATABASE_URL が設定されていない\n{}",
            HeadquartersAccountArgs::USAGE
        )
    })?;
    let analytics = AnalyticsDestination::from_env()?;
    let pool = PgPool::connect(&url)
        .await
        .map_err(|e| format!("DATABASE_URL に接続できない: {e}"))?;
    infra::MIGRATOR
        .run(&pool)
        .await
        .map_err(|e| format!("マイグレーションを当てられない: {e}"))?;

    let register = RegisterAccount::new(PgAccountRepository::new(pool), analytics, SystemClock);
    let account = HeadquartersAccount::ensure(&register, &args).await?;
    let account_id = account_id_uuid::format(account.account_id);
    if account.created {
        eprintln!("本部のアカウントを登録した: {account_id}");
    } else {
        eprintln!("同じメールアドレスの本部のアカウントがあるので何もしない: {account_id}");
    }
    println!("{account_id}");
    Ok(())
}
