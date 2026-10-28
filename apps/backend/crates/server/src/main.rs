//! HTTP サーバーの起動。待ち受けるアドレスは `BIND_ADDR`（既定 `127.0.0.1:8080`）、行動イベントの配信先は
//! `ANALYTICS_SINK`（既定 `memory`。[`server::AnalyticsDestination`]）、通話基盤は `VIDEO_CALL_PROVIDER`（既定 `twilio`。
//! [`server::VideoCallBackend`]）。

use std::{env, io};

use application::clock::SystemClock;
use server::{AnalyticsDestination, VideoCallBackend};

#[tokio::main]
async fn main() -> io::Result<()> {
    let addr = env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let analytics = AnalyticsDestination::from_env().map_err(io::Error::other)?;
    let video = VideoCallBackend::from_env().map_err(io::Error::other)?;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("listening on {}", listener.local_addr()?);
    axum::serve(
        listener,
        server::router_with_video_call(SystemClock, analytics, video),
    )
    .await
}
