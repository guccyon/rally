//! `access-token` のバイナリが、サーバーと同じくアクセストークンの署名の鍵を必須にすること（Issue #1785）。
//! 鍵がなく、署名なしのモックの opt-in（`ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK=1`）もなければ、DB につなぐ前に原因を書いて
//! 0 以外で終わる。DB は要らない。

use std::process::{Command, Output, Stdio};

/// `access-token --hints` を、親のトークンの設定と `DATABASE_URL` を渡さずに `envs` だけを足して動かす。
fn run(envs: &[(&str, &str)]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_access-token"))
        .arg("--hints")
        .env_remove("ACCESS_TOKEN_SIGNING_KEY")
        .env_remove("ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK")
        .env_remove("DATABASE_URL")
        .envs(envs.iter().copied())
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn fails_without_the_signing_key_or_the_opt_in() {
    let output = run(&[("DATABASE_URL", "postgres://unused.invalid/rally")]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "トークンを出さない");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("ACCESS_TOKEN_SIGNING_KEY がない"),
        "{stderr}"
    );
}

#[test]
fn gets_past_the_token_setup_with_the_key_or_the_opt_in() {
    // トークンの実装を選べたあとは、DATABASE_URL がないことで止まる（鍵の検査は通っている）
    for envs in [
        &[(
            "ACCESS_TOKEN_SIGNING_KEY",
            "seed-test-signing-key-0123456789abcdef",
        )][..],
        &[("ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK", "1")][..],
    ] {
        let output = run(envs);
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("DATABASE_URL が設定されていない"),
            "{stderr}"
        );
    }
}
