//! HTTP サーバーの起動。待ち受けるアドレスは `BIND_ADDR`（既定 `127.0.0.1:8080`）、行動イベントの配信先は
//! `ANALYTICS_SINK`（既定 `memory`。[`server::AnalyticsDestination`]）、通話基盤は `VIDEO_CALL_PROVIDER`（既定 `twilio`。
//! [`server::VideoCallBackend`]）。
//!
//! `DATABASE_URL` があれば保存先を Postgres にし（[`server::router_with_postgres_and_video_call`]）、待ち受ける前に
//! [`infra::run_migrations`] でマイグレーションを当てる。なければ保存先はインメモリ（[`server::router_with_video_call`]）。
//! `GET /version` は `APP_VERSION` の版を返す（[`server::with_app_version`]）。
//!
//! `CLOCK_OFFSET_SECONDS`（既定 0）は、サーバーの時計を実際より進める秒数（[`application::offset_clock::OffsetClock`]）。
//! 触れる環境（staging）で QA が「翌日」をまたぐ流れを確かめるために、工場の shipper だけが渡す。本番では渡さない。
//!
//! 起動できないとき（設定の値が知らないもの・時計を進める秒数が 0 以上の整数でない・DB につなげない・マイグレーションを当てられない）は、原因を標準エラー出力に
//! 書いて 0 以外で終わる。

use std::{env, process::ExitCode, time::Duration};

use application::offset_clock::OffsetClock;
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
    let clock = clock_from(env::var("CLOCK_OFFSET_SECONDS").ok().as_deref())?;
    if clock.offset() > Duration::ZERO {
        println!("clock is {} seconds ahead", clock.offset().as_secs());
    }
    let router = match env::var("DATABASE_URL") {
        Ok(url) => {
            let pool = PgPool::connect(&url)
                .await
                .map_err(|error| format!("DATABASE_URL の Postgres につなげない: {error}"))?;
            // 前の版に戻したとき（新しい版が当てたマイグレーションが DB に残る）も起動できるよう、知らない版は飛ばす
            infra::run_migrations(&pool)
                .await
                .map_err(|error| format!("マイグレーションを当てられない: {error}"))?;
            server::router_with_postgres_and_video_call(pool, clock, analytics, video)
        }
        Err(_) => server::router_with_video_call(clock, analytics, video),
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

/// `CLOCK_OFFSET_SECONDS` の値から時計を作る。ないか空なら進めない。
fn clock_from(value: Option<&str>) -> Result<OffsetClock, String> {
    match value.map(str::trim) {
        None | Some("") => Ok(OffsetClock::default()),
        Some(text) => text
            .parse::<u64>()
            .map(|seconds| OffsetClock::new(Duration::from_secs(seconds)))
            .map_err(|_| format!("CLOCK_OFFSET_SECONDS は 0 以上の整数（秒）: {text:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_offset_when_unset_or_empty() {
        assert_eq!(clock_from(None), Ok(OffsetClock::default()));
        assert_eq!(clock_from(Some("")), Ok(OffsetClock::default()));
    }

    #[test]
    fn offset_in_seconds() {
        assert_eq!(
            clock_from(Some("86400")),
            Ok(OffsetClock::new(Duration::from_secs(86_400)))
        );
    }

    #[test]
    fn rejects_negative_or_non_numbers() {
        assert!(clock_from(Some("-1")).is_err());
        assert!(clock_from(Some("1d")).is_err());
    }
}
