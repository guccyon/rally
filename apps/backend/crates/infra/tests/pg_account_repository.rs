//! `PgAccountRepository`（`identity.accounts`・`coaches`・`coach_sports`）の検証（生徒の表示名の書き換えと、profile の
//! `CoachProfileRepository` としてのコーチの自己紹介文 `coaches.bio` の読み書きを含む）。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::{
    identity::{AccountRepository, RegisterAccount, RegisterAccountCommand, RegisterAccountError},
    in_memory::{FixedClock, RecordingAnalyticsSink},
    profile::CoachProfileRepository,
};
use domain::{
    catalog::Sport,
    identity::{Account, AccountId, Email, Role},
    profile::{CoachBio, DisplayName},
};
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
        .execute(command(
            "taro@example.com",
            Role::Student,
            vec![Sport::Soccer],
        ))
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
        .execute(command(
            "taro@example.com",
            Role::Student,
            vec![Sport::Tennis],
        ))
        .await
        .unwrap();

    let result = use_case
        .execute(command(
            "TARO@example.com",
            Role::Coach,
            vec![Sport::Tennis],
        ))
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

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn coaches_are_listed_with_their_sports_in_code_point_order_of_display_names(pool: PgPool) {
    let use_case = register_account(&pool);
    let register = |email: &'static str, display_name: &'static str, role, sports| {
        let use_case = &use_case;
        async move {
            use_case
                .execute(RegisterAccountCommand {
                    display_name: display_name.to_string(),
                    ..command(email, role, sports)
                })
                .await
                .unwrap()
        }
    };
    // 照合順序に依らずコードポイントの順（"B" < "a" < "あ"）。同じ表示名は ID の順
    let hiragana = register("a@example.com", "あ", Role::Coach, vec![Sport::Golf]).await;
    let lower = register("b@example.com", "a", Role::Coach, vec![Sport::Tennis]).await;
    let upper = register(
        "c@example.com",
        "B",
        Role::Coach,
        vec![Sport::Soccer, Sport::Baseball],
    )
    .await;
    let same_1 = register("d@example.com", "あ", Role::Coach, vec![Sport::Tennis]).await;
    register("e@example.com", "0", Role::Student, vec![Sport::Tennis]).await;
    register("f@example.com", "1", Role::Headquarters, vec![]).await;

    let coaches = PgAccountRepository::new(pool).list_coaches().await.unwrap();
    let mut same_name = [hiragana.clone(), same_1.clone()];
    same_name.sort_by_key(|a| a.id());
    assert_eq!(
        coaches,
        [upper, lower, same_name[0].clone(), same_name[1].clone()]
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn no_coaches_is_an_empty_list(pool: PgPool) {
    register_account(&pool)
        .execute(command(
            "taro@example.com",
            Role::Student,
            vec![Sport::Tennis],
        ))
        .await
        .unwrap();

    let coaches = PgAccountRepository::new(pool).list_coaches().await.unwrap();
    assert!(coaches.is_empty());
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn students_are_listed_with_their_sport_newest_registration_first(pool: PgPool) {
    let accounts = PgAccountRepository::new(pool);
    let account = |id: u128, role, sports: &[Sport], registered_hour| {
        Account::new(
            AccountId::from_u128(id),
            Email::new(format!("user{id}@example.com")).unwrap(),
            DisplayName::new(format!("なまえ{id}")).unwrap(),
            role,
            sports.iter().copied(),
            support::at(registered_hour),
        )
        .unwrap()
    };
    let old = account(1, Role::Student, &[Sport::Tennis], 1);
    let newest = account(2, Role::Student, &[Sport::Swimming], 3);
    // 同じ登録時刻は ID の順
    let same_2 = account(4, Role::Student, &[Sport::Golf], 2);
    let same_1 = account(3, Role::Student, &[Sport::Soccer], 2);
    for account in [
        old.clone(),
        newest.clone(),
        same_2.clone(),
        same_1.clone(),
        account(5, Role::Coach, &[Sport::Tennis], 9),
        account(6, Role::Headquarters, &[], 9),
    ] {
        accounts.insert(account).await.unwrap();
    }

    assert_eq!(
        accounts.list_students().await.unwrap(),
        [newest, same_1, same_2, old]
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn no_students_is_an_empty_list(pool: PgPool) {
    register_account(&pool)
        .execute(command(
            "coach@example.com",
            Role::Coach,
            vec![Sport::Tennis],
        ))
        .await
        .unwrap();

    let students = PgAccountRepository::new(pool)
        .list_students()
        .await
        .unwrap();
    assert!(students.is_empty());
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn accounts_of_each_role_are_found_by_id_with_their_sports(pool: PgPool) {
    let use_case = register_account(&pool);
    let student = use_case
        .execute(command(
            "taro@example.com",
            Role::Student,
            vec![Sport::Soccer],
        ))
        .await
        .unwrap();
    let coach = use_case
        .execute(command(
            "coach@example.com",
            Role::Coach,
            vec![Sport::Tennis, Sport::Golf],
        ))
        .await
        .unwrap();
    let headquarters = use_case
        .execute(command("hq@example.com", Role::Headquarters, vec![]))
        .await
        .unwrap();

    let accounts = PgAccountRepository::new(pool);
    for account in [student, coach, headquarters] {
        assert_eq!(accounts.find(account.id()).await.unwrap(), Some(account));
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn unknown_account_is_none(pool: PgPool) {
    let found = PgAccountRepository::new(pool)
        .find(domain::identity::AccountId::from_u128(42))
        .await
        .unwrap();
    assert_eq!(found, None);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn coach_bio_is_empty_at_first_and_replaced_by_update(pool: PgPool) {
    let use_case = register_account(&pool);
    let coach = use_case
        .execute(command(
            "coach@example.com",
            Role::Coach,
            vec![Sport::Tennis, Sport::Golf],
        ))
        .await
        .unwrap();
    let accounts = PgAccountRepository::new(pool.clone());

    // 登録したばかりのコーチの自己紹介文は空
    assert_eq!(
        accounts.find_coach_bio(coach.id()).await.unwrap(),
        Some(CoachBio::default())
    );

    // 上限ちょうど（400 文字。DB の char_length も文字で数える）まで書け、読み直しても同じ
    for text in [
        "テニス歴 20 年。初心者歓迎",
        &"あ".repeat(CoachBio::MAX_CHARS),
        "",
    ] {
        let bio = CoachBio::new(text).unwrap();
        let updated = accounts
            .update_coach_bio(coach.id(), bio.clone())
            .await
            .unwrap();
        assert_eq!(updated, Some(bio.clone()));
        assert_eq!(
            accounts.find_coach_bio(coach.id()).await.unwrap(),
            Some(bio)
        );
        assert_eq!(
            accounts.find(coach.id()).await.unwrap(),
            Some(coach.clone()),
            "アカウント（提供スポーツなど）はそのまま"
        );
        assert_eq!(
            accounts.list_coaches().await.unwrap(),
            std::slice::from_ref(&coach)
        );
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn bio_of_non_coach_or_unknown_account_is_neither_read_nor_updated(pool: PgPool) {
    let use_case = register_account(&pool);
    let student = use_case
        .execute(command(
            "taro@example.com",
            Role::Student,
            vec![Sport::Soccer],
        ))
        .await
        .unwrap();
    let headquarters = use_case
        .execute(command("hq@example.com", Role::Headquarters, vec![]))
        .await
        .unwrap();
    let accounts = PgAccountRepository::new(pool);

    for id in [
        student.id(),
        headquarters.id(),
        domain::identity::AccountId::from_u128(42),
    ] {
        let updated = accounts
            .update_coach_bio(id, CoachBio::new("自己紹介").unwrap())
            .await
            .unwrap();
        assert_eq!(updated, None, "{id:?}");
        assert_eq!(accounts.find_coach_bio(id).await.unwrap(), None, "{id:?}");
    }
    assert_eq!(accounts.find(student.id()).await.unwrap(), Some(student));
    assert_eq!(
        accounts.find(headquarters.id()).await.unwrap(),
        Some(headquarters)
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn student_display_name_is_replaced_by_update(pool: PgPool) {
    let student = register_account(&pool)
        .execute(command(
            "taro@example.com",
            Role::Student,
            vec![Sport::Soccer],
        ))
        .await
        .unwrap();
    let accounts = PgAccountRepository::new(pool);

    // 上限ちょうど（50 文字）まで書け、読み直しても同じ。メールアドレス・スポーツ・登録日時はそのまま
    for text in ["じろう", &"あ".repeat(DisplayName::MAX_CHARS)] {
        let display_name = DisplayName::new(text).unwrap();
        let updated = accounts
            .update_student_display_name(student.id(), display_name.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.display_name(), &display_name);
        assert_eq!(updated.email(), student.email());
        assert_eq!(updated.as_student(), student.as_student());
        assert_eq!(updated.registered_at(), student.registered_at());
        assert_eq!(accounts.find(student.id()).await.unwrap(), Some(updated));
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn display_name_of_non_student_or_unknown_account_is_not_updated(pool: PgPool) {
    let use_case = register_account(&pool);
    let coach = use_case
        .execute(command(
            "coach@example.com",
            Role::Coach,
            vec![Sport::Tennis],
        ))
        .await
        .unwrap();
    let headquarters = use_case
        .execute(command("hq@example.com", Role::Headquarters, vec![]))
        .await
        .unwrap();
    let accounts = PgAccountRepository::new(pool);

    for id in [
        coach.id(),
        headquarters.id(),
        domain::identity::AccountId::from_u128(42),
    ] {
        let updated = accounts
            .update_student_display_name(id, DisplayName::new("じろう").unwrap())
            .await
            .unwrap();
        assert_eq!(updated, None, "{id:?}");
    }
    assert_eq!(accounts.find(coach.id()).await.unwrap(), Some(coach));
    assert_eq!(
        accounts.find(headquarters.id()).await.unwrap(),
        Some(headquarters)
    );
}
