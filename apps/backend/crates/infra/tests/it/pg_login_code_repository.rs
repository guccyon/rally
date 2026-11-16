//! `PgAccountRepository` の `LoginCodeRepository`（`identity.login_codes`）の検証。メールアドレスごとに 1 つで送り直すと
//! 置き換わること、間違いの数え上げと使い終わりが読んだときのコードにだけ効くことを確かめる。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

use crate::support;

use application::identity::LoginCodeRepository;
use domain::{
    catalog::Sport,
    identity::{Email, LoginCode, LoginCodeChallenge, Role},
};
use infra::PgAccountRepository;
use sqlx::PgPool;

fn email(s: &str) -> Email {
    Email::new(s).unwrap()
}

fn code(s: &str) -> LoginCode {
    LoginCode::new(s).unwrap()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_code_is_kept_per_email_and_replaced_by_resending(pool: PgPool) {
    let student = support::register(&pool, Role::Student, vec![Sport::Tennis]).await;
    let repository = PgAccountRepository::new(pool);
    assert_eq!(
        repository
            .find_login_code(&email("taro@example.com"))
            .await
            .unwrap(),
        None
    );

    let first = LoginCodeChallenge::issue(
        email("Taro@Example.com"),
        Some(student),
        code("012345"),
        support::at(0),
    );
    repository.save_login_code(first.clone()).await.unwrap();
    assert_eq!(
        repository
            .find_login_code(&email("taro@example.com"))
            .await
            .unwrap(),
        Some(first.clone())
    );

    let unknown = LoginCodeChallenge::issue(
        email("nobody@example.com"),
        None,
        code("999999"),
        support::at(0),
    );
    repository.save_login_code(unknown.clone()).await.unwrap();
    assert_eq!(
        repository
            .find_login_code(&email("NOBODY@example.com"))
            .await
            .unwrap(),
        Some(unknown)
    );

    repository
        .record_failed_login_code_attempt(&first)
        .await
        .unwrap();
    repository
        .record_failed_login_code_attempt(&first)
        .await
        .unwrap();
    let counted = repository
        .find_login_code(&email("taro@example.com"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(counted.failed_attempts(), 2);

    // 送り直すと置き換わり、前のコードの数え上げ・使い終わりは新しいコードに効かない
    let second = LoginCodeChallenge::issue(
        email("taro@example.com"),
        Some(student),
        code("123456"),
        support::at(1),
    );
    repository.save_login_code(second.clone()).await.unwrap();
    repository
        .record_failed_login_code_attempt(&counted)
        .await
        .unwrap();
    assert!(!repository.consume_login_code(&counted).await.unwrap());
    assert_eq!(
        repository
            .find_login_code(&email("taro@example.com"))
            .await
            .unwrap(),
        Some(second.clone())
    );

    assert!(repository.consume_login_code(&second).await.unwrap());
    assert!(!repository.consume_login_code(&second).await.unwrap());
    assert_eq!(
        repository
            .find_login_code(&email("taro@example.com"))
            .await
            .unwrap(),
        None
    );
}
