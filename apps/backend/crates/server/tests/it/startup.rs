//! サーバーのバイナリ（`main`）の起動。`DATABASE_URL` があれば Postgres につないでマイグレーションを当ててから待ち受け、
//! なければインメモリで待ち受ける。DB に当たっていて自分の知らないマイグレーション（新しい版が当てたもの）があっても
//! 起動し（前の版に戻したとき）、DB につなげない・マイグレーションを当てられないときは原因を標準エラー出力に書いて
//! 0 以外で終わる。アクセストークンの署名の鍵がなく、署名なしのモックの opt-in もなければ起動しない（Issue #1785）。
//!
//! バイナリを子プロセスで起動し（`BIND_ADDR=127.0.0.1:0`。標準出力の `listening on` の行から番地を読む）、HTTP で確かめる。
//! Postgres を使うものは既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test it startup:: -- --include-ignored` で走らせる。テストごとに `#[sqlx::test]` が作る DB に向ける。

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
        self.request_as(None, method, path, body)
    }

    /// `token` があれば `Authorization: Bearer` を付けて、[`Self::request`] と同じく 1 回だけ要求する。
    fn request_as(
        &self,
        token: Option<&str>,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> (u16, String) {
        let body = body.map(|b| b.to_string()).unwrap_or_default();
        let authorization = token
            .map(|token| format!("Authorization: Bearer {token}\r\n"))
            .unwrap_or_default();
        let mut stream = TcpStream::connect(&self.addr).unwrap();
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n{authorization}\
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

/// バイナリを起動するコマンド。親の `DATABASE_URL`・`APP_VERSION`・`ACCESS_TOKEN_SIGNING_KEY`・
/// `ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK` は渡さず、`envs` だけを足す。
fn command(envs: &[(&str, &str)]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_server"));
    command
        .env_remove("DATABASE_URL")
        .env_remove("APP_VERSION")
        .env_remove("ACCESS_TOKEN_SIGNING_KEY")
        .env_remove("ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK")
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

/// 署名なしのモックのトークンで動かす opt-in（ローカルの起動とテスト用）。
const UNSIGNED_MOCK: (&str, &str) = ("ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK", "1");

/// テストだけの署名の鍵（秘密ではない）。
const TEST_KEY: &str = "startup-test-signing-key-0123456789abcdef";

const UNKNOWN_VERSION: i64 = 99_991_231_000_001;

fn register_body() -> Value {
    json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "tennis" })
}

#[test]
fn starts_in_memory_without_database_url() {
    let server = Server::start(&[UNSIGNED_MOCK]);

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
    let stderr = fails_to_start(&[("DATABASE_URL", "not-a-postgres-url"), UNSIGNED_MOCK]);

    assert!(
        stderr.contains("DATABASE_URL の Postgres につなげない"),
        "{stderr}"
    );
}

#[test]
fn issues_signed_tokens_when_the_signing_key_is_set() {
    let server = Server::start(&[("ACCESS_TOKEN_SIGNING_KEY", TEST_KEY)]);

    let (status, body) = server.request("POST", "/accounts", Some(register_body()));
    assert_eq!(status, 201, "{body}");
    let body: Value = serde_json::from_str(&body).unwrap();
    let token = body["access_token"].as_str().unwrap();
    assert!(token.starts_with("v1."), "{token}");
}

#[test]
fn exits_with_the_cause_without_the_signing_key_or_the_opt_in() {
    // 鍵が配られていない環境は起動しない（黙って署名なしのモックで動かない。Issue #1785）
    for envs in [&[][..], &[("ACCESS_TOKEN_SIGNING_KEY", "")][..]] {
        let stderr = fails_to_start(envs);
        assert!(
            stderr.contains("ACCESS_TOKEN_SIGNING_KEY がない"),
            "{stderr}"
        );
        assert!(
            stderr.contains("ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK=1"),
            "{stderr}"
        );
    }
}

#[test]
fn exits_with_the_cause_when_the_opt_in_is_not_1() {
    let stderr = fails_to_start(&[("ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK", "true")]);

    assert!(
        stderr.contains("ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK は 1 だけ"),
        "{stderr}"
    );
}

#[test]
fn issues_unsigned_mock_tokens_only_with_the_opt_in() {
    let mut child = command(&[UNSIGNED_MOCK])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stderr.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let _ = child.kill();
    let _ = child.wait();
    assert!(
        line.contains("access tokens are unsigned mock tokens"),
        "{line:?}"
    );

    let server = Server::start(&[UNSIGNED_MOCK]);
    let (status, body) = server.request("POST", "/accounts", Some(register_body()));
    assert_eq!(status, 201, "{body}");
    let body: Value = serde_json::from_str(&body).unwrap();
    let token = body["access_token"].as_str().unwrap();
    assert!(token.starts_with("mock.student."), "{token}");
}

#[test]
fn the_signing_key_wins_and_a_student_cannot_forge_a_coach_token() {
    // 鍵と opt-in が両方あれば鍵で署名する。生徒が見えるコーチの ID から作った `mock.coach.<ID>` では、
    // コーチの予約の一覧（ほかの生徒の予約の経緯）を読めない（Issue #1785 の再現）
    let server = Server::start(&[("ACCESS_TOKEN_SIGNING_KEY", TEST_KEY), UNSIGNED_MOCK]);

    let (status, body) = server.request("POST", "/accounts", Some(register_body()));
    assert_eq!(status, 201, "{body}");
    let body: Value = serde_json::from_str(&body).unwrap();
    let student = body["access_token"].as_str().unwrap().to_string();
    assert!(student.starts_with("v1."), "{student}");
    assert_eq!(server.request_as(Some(&student), "GET", "/me", None).0, 200);

    let forged = "mock.coach.33b2bb51-054b-48b0-8f2e-418048c57045";
    let (status, body) = server.request_as(Some(forged), "GET", "/coaches/me/bookings", None);
    assert_eq!(status, 401, "{body}");
    // 生徒自身の署名付きトークンでも、コーチの一覧は役割が合わず通らない
    let (status, body) = server.request_as(Some(&student), "GET", "/coaches/me/bookings", None);
    assert_eq!(status, 403, "{body}");
}

#[test]
fn deploy_compose_requires_the_signing_key_and_never_opts_in_to_the_mock() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../deploy/compose.yaml"
    );
    let compose = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert!(
        compose.contains("ACCESS_TOKEN_SIGNING_KEY: ${ACCESS_TOKEN_SIGNING_KEY:?"),
        "鍵がなければ compose が起動を止める: {compose}"
    );
    // コメントの説明は除き、設定の行に opt-in がないこと
    let settings: Vec<&str> = compose
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect();
    assert!(
        settings
            .iter()
            .all(|line| !line.contains("ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK")),
        "出荷する環境には署名なしのモックの opt-in を渡さない"
    );
}

#[test]
fn exits_with_the_cause_when_the_signing_key_is_short() {
    let stderr = fails_to_start(&[("ACCESS_TOKEN_SIGNING_KEY", "short-key")]);

    assert!(
        stderr.contains("ACCESS_TOKEN_SIGNING_KEY は 32 バイト以上"),
        "{stderr}"
    );
    assert!(!stderr.contains("short-key"), "鍵の値を出さない: {stderr}");
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrations = false)]
async fn migrates_an_empty_database_then_serves_from_postgres(pool: PgPool) {
    let url = database_url(&pool).await;
    let server = Server::start(&[
        ("DATABASE_URL", &url),
        ("APP_VERSION", "0.3.1"),
        UNSIGNED_MOCK,
    ]);

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
    let first = Server::start(&[("DATABASE_URL", &url), UNSIGNED_MOCK]);
    assert_eq!(
        first.request("POST", "/accounts", Some(register_body())).0,
        201
    );
    drop(first);

    let second = Server::start(&[("DATABASE_URL", &url), UNSIGNED_MOCK]);

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

    let server = Server::start(&[("DATABASE_URL", &url), UNSIGNED_MOCK]);

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

    let stderr = fails_to_start(&[("DATABASE_URL", &url), UNSIGNED_MOCK]);

    assert!(
        stderr.contains("マイグレーションを当てられない"),
        "{stderr}"
    );
    assert!(stderr.contains(&first.to_string()), "{stderr}");
}
