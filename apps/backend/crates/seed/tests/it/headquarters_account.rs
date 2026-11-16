//! 本部のアカウントを作る `headquarters-account`（`crates/seed/src/bin/headquarters-account.rs`。Issue #1579）を、
//! テストごとの新しい DB に向けて実際に 2 回動かし、1 回目が本部のアカウントを登録してその ID を標準出力に書き、同じメール
//! アドレス（大文字・小文字を問わない）の 2 回目は何もせず成功して同じ ID を書くこと（冪等）を確かめる。そのメールアドレスを
//! 本部以外が使っていれば失敗し、何も書かないことも確かめる。重なりの判定の細かい検査は `seed::HeadquartersAccount` の単体の
//! テスト。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p seed -- --include-ignored` で走らせる。

use std::{
    process::{Command, Output},
    time::SystemTime,
};

use application::{
    identity::{RegisterAccount, RegisterAccountCommand},
    in_memory::{FixedClock, RecordingAnalyticsSink},
};
use domain::{catalog::Sport, identity::Role};
use infra::PgAccountRepository;
use sqlx::{PgPool, types::Uuid};

/// テストの DB（`pool` の DB）を指す接続文字列。`DATABASE_URL` の DB 名だけを差し替える。
fn database_url(pool: &PgPool) -> String {
    let base = std::env::var("DATABASE_URL").unwrap();
    let database = pool.connect_options().get_database().unwrap().to_string();
    let (without_query, query) = base
        .split_once('?')
        .map_or((base.as_str(), None), |(u, q)| (u, Some(q)));
    let scheme_end = without_query.find("://").map_or(0, |i| i + 3);
    let path_start = without_query[scheme_end..]
        .find('/')
        .map_or(without_query.len(), |i| scheme_end + i);
    let mut url = format!("{}/{database}", &without_query[..path_start]);
    if let Some(query) = query {
        url = format!("{url}?{query}");
    }
    url
}

fn run(pool: &PgPool, email: &str, display_name: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_headquarters-account"))
        .args(["--email", email, "--display-name", display_name])
        .env("DATABASE_URL", database_url(pool))
        .env_remove("ANALYTICS_SINK")
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).unwrap()
}

async fn accounts(pool: &PgPool) -> Vec<(Uuid, String, String, String)> {
    sqlx::query_as(
        "SELECT account_id, role, email, display_name FROM identity.accounts ORDER BY created_at",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn second_run_with_the_same_email_does_nothing_and_prints_the_same_id(pool: PgPool) {
    let first = stdout(&run(&pool, "hq@example.com", "本部"));
    let second = stdout(&run(&pool, "HQ@Example.com", "別の名前"));

    let id: Uuid = first.trim_end().parse().unwrap();
    assert_eq!(
        first,
        format!("{id}\n"),
        "標準出力はアカウント ID の 1 行だけ"
    );
    assert_eq!(second, first);
    assert_eq!(
        accounts(&pool).await,
        [(
            id,
            "headquarters".to_string(),
            "hq@example.com".to_string(),
            "本部".to_string()
        )]
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn email_of_a_coach_or_invalid_input_fails_without_writing(pool: PgPool) {
    RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        FixedClock(SystemTime::UNIX_EPOCH),
    )
    .execute(RegisterAccountCommand {
        email: "coach@example.com".to_string(),
        display_name: "コーチ".to_string(),
        role: Role::Coach,
        sports: vec![Sport::Tennis],
    })
    .await
    .unwrap();
    let before = accounts(&pool).await;

    for output in [
        run(&pool, "Coach@example.com", "本部"),
        run(&pool, "not-an-email", "本部"),
        run(&pool, "other@example.com", ""),
    ] {
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    assert_eq!(accounts(&pool).await, before);
}
