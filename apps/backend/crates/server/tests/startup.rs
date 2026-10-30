//! サーバーのバイナリ（`main`）の起動。`DATABASE_URL` があれば Postgres につないでマイグレーションを当ててから待ち受け、
//! なければインメモリで待ち受ける。DB に当たっていて自分の知らないマイグレーション（新しい版が当てたもの）があっても
//! 起動し（前の版に戻したとき）、DB につなげない・マイグレーションを当てられないときは原因を標準エラー出力に書いて
//! 0 以外で終わる。
//!
//! バイナリを子プロセスで起動し（`BIND_ADDR=127.0.0.1:0`。標準出力の `listening on` の行から番地を読む）、HTTP で確かめる。
//! Postgres を使うものは既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test startup -- --include-ignored` で走らせる。テストごとに `#[sqlx::test]` が作る DB に向ける。

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    process::{Child, Command, Stdio},
};

use serde_json::{Value, json};
use sqlx::{Connection, PgConnection, PgPool};

/// 起動したサーバーの子プロセス。落とすときに止める。
struct Server {
    child: Child,
    addr: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Server {
    /// `envs` を足してバイナリを起動し、待ち受けを始めるまで待つ。`DATABASE_URL` は `envs` にあるときだけ渡す。
    fn start(envs: &[(&str, &str)]) -> Self {
        let mut child = command(envs).stdout(Stdio::piped()).spawn().unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let addr = line
            .trim()
            .strip_prefix("listening on ")
            .unwrap_or_else(|| panic!("待ち受けを始めていない: {line:?}"))
            .to_string();
        Self { child, addr }
    }

    /// HTTP/1.1 で 1 回だけ要求して、status と本文を返す。
    fn request(&self, method: &str, path: &str, body: Option<Value>) -> (u16, String) {
        let body = body.map(|b| b.to_string()).unwrap_or_default();
        let mut stream = TcpStream::connect(&self.addr).unwrap();
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (head, body) = response.split_once("\r\n\r\n").unwrap();
        let status = head.split(' ').nth(1).unwrap().parse().unwrap();
        (status, body.to_string())
    }
}

/// バイナリを起動するコマンド。親の `DATABASE_URL`・`APP_VERSION` は渡さず、`envs` だけを足す。
fn command(envs: &[(&str, &str)]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_server"));
    command
        .env_remove("DATABASE_URL")
        .env_remove("APP_VERSION")
        .env("BIND_ADDR", "127.0.0.1:0")
        .envs(envs.iter().copied())
        .stdin(Stdio::null());
    command
}

/// 起動できずに終わることを確かめて、標準エラー出力を返す。
fn fails_to_start(envs: &[(&str, &str)]) -> String {
    let output = command(envs).output().unwrap();
    assert!(!output.status.success(), "起動できてしまった");
    String::from_utf8(output.stderr).unwrap()
}

/// `pool`（`#[sqlx::test]` が作った DB）につなぐ URL。`DATABASE_URL` の DB の名前だけを替える。
async fn database_url(pool: &PgPool) -> String {
    let name: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(pool)
        .await
        .unwrap();
    let base = std::env::var("DATABASE_URL").expect("DATABASE_URL が要る");
    let (base, query) = match base.split_once('?') {
        Some((base, query)) => (base.to_string(), format!("?{query}")),
        None => (base, String::new()),
    };
    let (server, _) = base.rsplit_once('/').unwrap();
    format!("{server}/{name}{query}")
}

async fn applied_versions(pool: &PgPool) -> Vec<i64> {
    sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
        .fetch_all(pool)
        .await
        .unwrap()
}

/// 新しい版が当てたことにするマイグレーション（この版の `infra::MIGRATOR` にない版）を DB に書き込む。
async fn record_unknown_migration(pool: &PgPool, version: i64) {
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
         VALUES ($1, 'expand_from_a_newer_release', true, '\\x00'::bytea, 0)",
    )
    .bind(version)
    .execute(pool)
    .await
    .unwrap();
}

const UNKNOWN_VERSION: i64 = 99_991_231_000_001;

fn register_body() -> Value {
    json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "tennis" })
}

#[test]
fn starts_in_memory_without_database_url() {
    let server = Server::start(&[]);

    assert_eq!(server.request("GET", "/healthz", None), (200, "ok".into()));
    let (status, body) = server.request("GET", "/version", None);
    assert_eq!(status, 200);
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap(),
        json!({ "app_version": "unknown" })
    );
    assert_eq!(
        server.request("POST", "/accounts", Some(register_body())).0,
        201
    );
}

#[test]
fn exits_with_the_cause_when_database_url_is_invalid() {
    let stderr = fails_to_start(&[("DATABASE_URL", "not-a-postgres-url")]);

    assert!(
        stderr.contains("DATABASE_URL の Postgres につなげない"),
        "{stderr}"
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrations = false)]
async fn migrates_an_empty_database_then_serves_from_postgres(pool: PgPool) {
    let url = database_url(&pool).await;
    let server = Server::start(&[("DATABASE_URL", &url), ("APP_VERSION", "0.3.1")]);

    let known: Vec<i64> = infra::MIGRATOR.iter().map(|m| m.version).collect();
    assert_eq!(applied_versions(&pool).await, known);
    assert_eq!(server.request("GET", "/healthz", None), (200, "ok".into()));
    let (_, body) = server.request("GET", "/version", None);
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap(),
        json!({ "app_version": "0.3.1" })
    );
    let (status, body) = server.request("POST", "/accounts", Some(register_body()));
    assert_eq!(status, 201, "{body}");
    let accounts: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(accounts, 1, "登録が Postgres に保存されていない");
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrations = false)]
async fn restart_on_a_migrated_database_applies_nothing_and_keeps_the_data(pool: PgPool) {
    let url = database_url(&pool).await;
    let first = Server::start(&[("DATABASE_URL", &url)]);
    assert_eq!(
        first.request("POST", "/accounts", Some(register_body())).0,
        201
    );
    drop(first);

    let second = Server::start(&[("DATABASE_URL", &url)]);

    let known: Vec<i64> = infra::MIGRATOR.iter().map(|m| m.version).collect();
    assert_eq!(applied_versions(&pool).await, known);
    // 同じメールは前の起動で登録済み
    assert_eq!(
        second.request("POST", "/accounts", Some(register_body())).0,
        409
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn starts_when_a_newer_release_applied_a_migration_it_does_not_know(pool: PgPool) {
    record_unknown_migration(&pool, UNKNOWN_VERSION).await;
    let url = database_url(&pool).await;
    // 既定の sqlx の Migrator は知らない版で止まる（この試験が前の版に戻した状況を作れていること）。失敗した Migrator は
    // advisory lock を解かずに返るので、プールの接続でなく使い捨ての接続で確かめて閉じる
    let mut conn = PgConnection::connect(&url).await.unwrap();
    let error = infra::MIGRATOR.run(&mut conn).await.unwrap_err();
    assert!(
        matches!(
            error,
            sqlx::migrate::MigrateError::VersionMissing(UNKNOWN_VERSION)
        ),
        "{error}"
    );
    conn.close().await.unwrap();

    let server = Server::start(&[("DATABASE_URL", &url)]);

    assert_eq!(server.request("GET", "/healthz", None), (200, "ok".into()));
    assert_eq!(
        server.request("POST", "/accounts", Some(register_body())).0,
        201
    );
    assert_eq!(
        applied_versions(&pool).await.last(),
        Some(&UNKNOWN_VERSION),
        "知らない版の記録は残す"
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn exits_with_the_cause_when_an_applied_migration_was_modified(pool: PgPool) {
    let first = infra::MIGRATOR.iter().next().unwrap().version;
    sqlx::query("UPDATE _sqlx_migrations SET checksum = '\\x00'::bytea WHERE version = $1")
        .bind(first)
        .execute(&pool)
        .await
        .unwrap();
    let url = database_url(&pool).await;

    let stderr = fails_to_start(&[("DATABASE_URL", &url)]);

    assert!(
        stderr.contains("マイグレーションを当てられない"),
        "{stderr}"
    );
    assert!(stderr.contains(&first.to_string()), "{stderr}");
}
