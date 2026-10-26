//! `PgAccountRepository`（`identity.accounts`・`coaches`・`coach_sports`）の検証。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::{
    identity::{RegisterAccount, RegisterAccountCommand, RegisterAccountError},
    in_memory::{FixedClock, RecordingAnalyticsSink},
};
use domain::identity::{Role, Sport};
use infra::PgAccountRepository;
use sqlx::{PgPool, types::Uuid};

fn register_account(
    pool: &PgPool,
) -> RegisterAccount<PgAccountRepository, RecordingAnalyticsSink, FixedClock> {
    RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        FixedClock(support::at(0)),
    )
}

fn command(email: &str, role: Role, sports: Vec<Sport>) -> RegisterAccountCommand {
    RegisterAccountCommand {
        email: email.to_string(),
        display_name: "たろう".to_string(),
        role,
        sports,
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn students_and_coaches_are_saved_with_their_sports(pool: PgPool) {
    let use_case = register_account(&pool);
    let student = use_case
        .execute(command("taro@example.com", Role::Student, vec![Sport::Soccer]))
        .await
        .unwrap();
    let coach = use_case
        .execute(command(
            "coach@example.com",
            Role::Coach,
            vec![Sport::Tennis, Sport::Basketball],
        ))
        .await
        .unwrap();

    let rows: Vec<(Uuid, String, Option<String>, time::OffsetDateTime)> = sqlx::query_as(
        "SELECT account_id, role, sport, created_at FROM identity.accounts ORDER BY role DESC",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            (
                application::identity::account_id_uuid::to_uuid(student.id()),
                "student".to_string(),
                Some("soccer".to_string()),
                support::at(0).into(),
            ),
            (
                application::identity::account_id_uuid::to_uuid(coach.id()),
                "coach".to_string(),
                None,
                support::at(0).into(),
            ),
        ]
    );
    let coach_sports: Vec<String> = sqlx::query_scalar(
        "SELECT sport FROM identity.coach_sports WHERE coach_id = $1 ORDER BY sport",
    )
    .bind(application::identity::account_id_uuid::to_uuid(coach.id()))
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(coach_sports, ["basketball", "tennis"]);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn duplicate_email_is_rejected_case_insensitively_and_nothing_is_saved(pool: PgPool) {
    let use_case = register_account(&pool);
    use_case
        .execute(command("taro@example.com", Role::Student, vec![Sport::Tennis]))
        .await
        .unwrap();

    let result = use_case
        .execute(command("TARO@example.com", Role::Coach, vec![Sport::Tennis]))
        .await;
    assert!(
        matches!(result, Err(RegisterAccountError::EmailAlreadyRegistered)),
        "{result:?}"
    );
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM identity.accounts), (SELECT count(*) FROM identity.coaches)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (1, 0));
}
