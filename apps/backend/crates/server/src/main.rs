//! HTTP サーバーの起動。待ち受けるアドレスは `BIND_ADDR`（既定 `127.0.0.1:8080`）。

use std::env;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let addr = env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("listening on {}", listener.local_addr()?);
    axum::serve(listener, server::router()).await
}
