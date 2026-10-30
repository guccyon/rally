//! HTTP サーバーの起動。待ち受けるアドレスは `BIND_ADDR`（既定 `127.0.0.1:8080`）、行動イベントの配信先は
//! `ANALYTICS_SINK`（既定 `memory`。[`server::AnalyticsDestination`]）、通話基盤は `VIDEO_CALL_PROVIDER`（既定 `twilio`。
//! [`server::VideoCallBackend`]）。
//!
//! `DATABASE_URL` があれば保存先を Postgres にし（[`server::router_with_postgres_and_video_call`]）、待ち受ける前に
//! [`infra::run_migrations`] でマイグレーションを当てる。なければ保存先はインメモリ（[`server::router_with_video_call`]）。
//! `GET /version` は `APP_VERSION` の版を返す（[`server::with_app_version`]）。
//!
//! 起動できないとき（設定の値が知らないもの・DB につなげない・マイグレーションを当てられない）は、原因を標準エラー出力に
//! 書いて 0 以外で終わる。

use std::{env, process::ExitCode};

use application::clock::SystemClock;
use server::{AnalyticsDestination, VideoCallBackend};
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
    let addr = env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let analytics = AnalyticsDestination::from_env()?;
    let video = VideoCallBackend::from_env().map_err(|error| error.to_string())?;
    let router = match env::var("DATABASE_URL") {
        Ok(url) => {
            let pool = PgPool::connect(&url)
                .await
                .map_err(|error| format!("DATABASE_URL の Postgres につなげない: {error}"))?;
            // 前の版に戻したとき（新しい版が当てたマイグレーションが DB に残る）も起動できるよう、知らない版は飛ばす
            infra::run_migrations(&pool)
                .await
                .map_err(|error| format!("マイグレーションを当てられない: {error}"))?;
            server::router_with_postgres_and_video_call(pool, SystemClock, analytics, video)
        }
        Err(_) => server::router_with_video_call(SystemClock, analytics, video),
    };
    let router = server::with_app_version(router, |name| env::var(name).ok());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|error| format!("{addr} で待ち受けられない: {error}"))?;
    let local_addr = listener.local_addr().map_err(|error| error.to_string())?;
    println!("listening on {local_addr}");
    axum::serve(listener, router)
        .await
        .map_err(|error| error.to_string())
}
