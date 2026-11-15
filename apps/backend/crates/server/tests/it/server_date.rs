//! 応答の `Date` ヘッダーをサーバーの時計で付ける（factory-engine の ADR 0055 の決定 2）の統合テスト。
//! 画面は `Date` でサーバーの時刻に合わせるので、時計を進めた触れる環境でも画面と API が食い違わないこと。

use std::time::{Duration, SystemTime};

use application::offset_clock::OffsetClock;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// 本物の HTTP の土台（hyper）を通して `GET /healthz` を送り、応答の `Date` の行をすべて返す。
async fn date_lines(clock: OffsetClock) -> Vec<String> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = server::with_server_date(server::router_at(clock), clock);
    tokio::spawn(async move { axum::serve(listener, router).await });

    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    response
        .lines()
        .filter(|line| line.to_ascii_lowercase().starts_with("date:"))
        .map(|line| line["date:".len()..].trim().to_string())
        .collect()
}

#[tokio::test]
async fn date_follows_the_advanced_clock() {
    let day = Duration::from_secs(86_400);
    let lines = date_lines(OffsetClock::new(day)).await;

    assert_eq!(
        lines.len(),
        1,
        "Date は 1 つだけ（土台が付け直さない）: {lines:?}"
    );
    let expected = [
        interface::server_date::http_date(SystemTime::now() + day),
        interface::server_date::http_date(SystemTime::now() + day - Duration::from_secs(2)),
    ];
    // 秒の境をまたいでもよいように、送る前後の 2 秒の幅で見る
    assert!(
        expected.iter().any(|e| e[..16] == lines[0][..16]),
        "{lines:?} は 1 日先の日付でない（{expected:?}）"
    );
}

#[tokio::test]
async fn date_is_the_real_time_without_offset() {
    let lines = date_lines(OffsetClock::default()).await;

    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(
        lines[0][..16],
        interface::server_date::http_date(SystemTime::now())[..16]
    );
}
