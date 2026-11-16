//! アクセストークンを発行する: `access-token --hints`・`access-token --account <アカウント ID>`
//! （`apps/backend/README.md` の「認証」。staging の runbook）。
//!
//! 環境変数 `DATABASE_URL` の Postgres からアカウントの役割を読み、サーバーと同じ実装（`ACCESS_TOKEN_SIGNING_KEY` が
//! あれば署名付き。[`infra::AccessTokenBackend`]）でトークンを発行して標準出力に書く。サーバーと同じ鍵を持つ
//! backend のコンテナの中で動かす（鍵を外に出さない）。DB に書かない（マイグレーションも当てない）。

use std::process::ExitCode;

use domain::identity::Role;
use infra::AccessTokenBackend;
use seed::{AccessTokenArgs, LoginHint};
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
    let args = AccessTokenArgs::parse(std::env::args().skip(1))
        .map_err(|e| format!("{e}\n{}", AccessTokenArgs::USAGE))?;
    let tokens = AccessTokenBackend::from_env().map_err(|e| e.to_string())?;
    let url = std::env::var("DATABASE_URL").map_err(|_| {
        format!(
            "DATABASE_URL が設定されていない\n{}",
            AccessTokenArgs::USAGE
        )
    })?;
    let pool = PgPool::connect(&url)
        .await
        .map_err(|e| format!("DATABASE_URL に接続できない: {e}"))?;
    match args {
        AccessTokenArgs::Hints => {
            let hints = LoginHint::load(&pool, &tokens)
                .await
                .map_err(|e| format!("アカウントを読めない: {e}"))?;
            LoginHint::check_complete(&hints).map_err(|e| {
                let missing: Vec<String> = e
                    .shortfalls
                    .iter()
                    .map(|(role, need, found)| {
                        let name = if *role == Role::Headquarters { "本部" } else { "コーチ" };
                        format!("{name}が {need} 人必要なのに {found} 人")
                    })
                    .collect();
                format!(
                    "ログインの手がかりが足りない: {}。DB に本部とコーチのアカウントがない。staging はシードから入れ直す（apps/backend/README.md の「staging でトークンを作る」）",
                    missing.join("、")
                )
            })?;
            let json: Vec<_> = hints.iter().map(LoginHint::to_json).collect();
            println!("{}", serde_json::Value::Array(json));
        }
        AccessTokenArgs::Account(id) => {
            let token = LoginHint::for_account(&pool, &tokens, id)
                .await
                .map_err(|e| format!("アカウントを読めない: {e}"))?
                .ok_or_else(|| format!("アカウント {id} は登録されていない"))?;
            println!("{token}");
        }
    }
    Ok(())
}
