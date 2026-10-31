//! `PgLessonMenuRepository`（`catalog.lesson_menus`）の検証。コーチのアカウントを `PgAccountRepository` で読んで
//! メニューを作り・直し・読み戻せること、公開中の一覧とスポーツの絞り込み、他のコーチのメニューを直せないこと、
//! 本部が見る横断の一覧（`ListLessonMenus`。コーチの表示名は `ProfileCoachDisplayNameLookup` で読む）の絞り込みと並べ方。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::{
    catalog::{
        CreateLessonMenu, CreateLessonMenuCommand, LessonMenuRepository, ListCoachLessonMenus,
        ListLessonMenus, ListMyLessonMenus, UpdateLessonMenu, UpdateLessonMenuCommand,
        UpdateLessonMenuError,
    },
    in_memory::FixedClock,
    profile::ProfileCoachDisplayNameLookup,
};
use domain::{
    DomainError,
    catalog::{
        LessonDuration, LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription,
        LessonMenuName,
    },
    identity::{AccountId, Role, Sport},
    lesson::LessonType,
};
use infra::{PgAccountRepository, PgLessonMenuRepository};
use sqlx::PgPool;
use support::at;

fn content(sport: Sport, format: LessonFormat, name: &str) -> LessonMenuContent {
    LessonMenuContent {
        sport,
        format,
        name: LessonMenuName::new(name).unwrap(),
        description: LessonMenuDescription::new("初心者向け。ラケットの持ち方から").unwrap(),
    }
}

fn live(minutes: i64) -> LessonFormat {
    LessonFormat::new(
        LessonType::Live,
        Some(LessonDuration::from_minutes(minutes).unwrap()),
    )
    .unwrap()
}

async fn create(
    pool: &PgPool,
    coach: AccountId,
    hour: u64,
    content: LessonMenuContent,
    is_published: bool,
) -> LessonMenu {
    CreateLessonMenu::new(
        PgLessonMenuRepository::new(pool.clone()),
        PgAccountRepository::new(pool.clone()),
        FixedClock(at(hour)),
    )
    .execute(CreateLessonMenuCommand {
        coach_id: coach,
        content,
        is_published,
    })
    .await
    .unwrap()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn creates_revises_and_reads_back_menus(pool: PgPool) {
    let coach = support::register(&pool, Role::Coach, vec![Sport::Tennis, Sport::Swimming]).await;
    let menus = PgLessonMenuRepository::new(pool.clone());

    let menu = create(
        &pool,
        coach,
        1,
        content(Sport::Tennis, live(45), "フォア改善"),
        true,
    )
    .await;
    assert_eq!(menus.find(menu.id()).await.unwrap(), Some(menu.clone()));

    // 非同期に直して非公開にする
    let updated = UpdateLessonMenu::new(menus.clone(), PgAccountRepository::new(pool.clone()))
        .execute(UpdateLessonMenuCommand {
            coach_id: coach,
            lesson_menu_id: menu.id(),
            content: content(Sport::Swimming, LessonFormat::Async, "クロールの添削"),
            is_published: false,
        })
        .await
        .unwrap();
    assert_eq!(menus.find(menu.id()).await.unwrap(), Some(updated.clone()));
    assert_eq!(updated.created_at(), menu.created_at());
    assert_eq!(updated.format().duration(), None);
    assert!(!updated.is_published());
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn lists_menus_of_the_coach_and_only_published_ones_for_students(pool: PgPool) {
    let coach = support::register(&pool, Role::Coach, vec![Sport::Tennis, Sport::Soccer]).await;
    let other = support::coach(&pool).await;
    let third = create(&pool, coach, 3, content(Sport::Tennis, live(60), "3"), true).await;
    let first = create(
        &pool,
        coach,
        1,
        content(Sport::Soccer, LessonFormat::Async, "1"),
        true,
    )
    .await;
    let hidden = create(
        &pool,
        coach,
        2,
        content(Sport::Tennis, live(30), "2"),
        false,
    )
    .await;
    create(
        &pool,
        other,
        0,
        content(Sport::Tennis, live(30), "他"),
        true,
    )
    .await;
    let menus = PgLessonMenuRepository::new(pool.clone());

    assert_eq!(
        ListMyLessonMenus::new(menus.clone())
            .execute(coach)
            .await
            .unwrap(),
        vec![first.clone(), hidden, third.clone()]
    );
    let published = ListCoachLessonMenus::new(menus);
    assert_eq!(
        published.execute(coach, None).await.unwrap(),
        vec![first, third.clone()]
    );
    assert_eq!(
        published.execute(coach, Some(Sport::Tennis)).await.unwrap(),
        vec![third]
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn another_coach_cannot_revise_a_menu(pool: PgPool) {
    let coach = support::coach(&pool).await;
    let other = support::coach(&pool).await;
    let menu = create(
        &pool,
        coach,
        1,
        content(Sport::Tennis, live(30), "フォア"),
        true,
    )
    .await;
    let menus = PgLessonMenuRepository::new(pool.clone());

    let result = UpdateLessonMenu::new(menus.clone(), PgAccountRepository::new(pool.clone()))
        .execute(UpdateLessonMenuCommand {
            coach_id: other,
            lesson_menu_id: menu.id(),
            content: content(Sport::Tennis, live(30), "乗っ取り"),
            is_published: false,
        })
        .await;
    assert!(
        matches!(
            result,
            Err(UpdateLessonMenuError::Invalid(
                DomainError::NotTheLessonMenuCoach
            ))
        ),
        "{result:?}"
    );
    assert_eq!(menus.find(menu.id()).await.unwrap(), Some(menu));
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn lists_menus_of_every_coach_by_sport_then_coach_name(pool: PgPool) {
    let tanaka = support::register_named(
        &pool,
        Role::Coach,
        vec![Sport::Tennis, Sport::Soccer],
        "たなか",
    )
    .await;
    let ito = support::register_named(&pool, Role::Coach, vec![Sport::Tennis], "いとう").await;
    let tanaka_soccer = create(
        &pool,
        tanaka,
        0,
        content(Sport::Soccer, LessonFormat::Async, "リフティング"),
        true,
    )
    .await;
    let tanaka_tennis = create(
        &pool,
        tanaka,
        1,
        content(Sport::Tennis, live(60), "サーブ"),
        true,
    )
    .await;
    let ito_later = create(
        &pool,
        ito,
        3,
        content(Sport::Tennis, LessonFormat::Async, "フォームの添削"),
        false,
    )
    .await;
    let ito_earlier = create(
        &pool,
        ito,
        2,
        content(Sport::Tennis, live(30), "フォア"),
        true,
    )
    .await;

    let use_case = ListLessonMenus::new(
        PgLessonMenuRepository::new(pool.clone()),
        ProfileCoachDisplayNameLookup::new(PgAccountRepository::new(pool.clone())),
    );
    let list = use_case.execute(None, None).await.unwrap();
    assert_eq!(
        list.lesson_menus,
        vec![
            ito_earlier.clone(),
            ito_later.clone(),
            tanaka_tennis.clone(),
            tanaka_soccer.clone(),
        ],
        "テニス（いとう → たなか）→ サッカー。同じコーチは作った時刻の古い順"
    );
    assert_eq!(
        list.coaches.get(&ito).map(|name| name.as_str()),
        Some("いとう")
    );
    assert_eq!(
        list.coaches.get(&tanaka).map(|name| name.as_str()),
        Some("たなか")
    );

    assert_eq!(
        use_case
            .execute(Some(Sport::Soccer), None)
            .await
            .unwrap()
            .lesson_menus,
        vec![tanaka_soccer]
    );
    assert_eq!(
        use_case
            .execute(None, Some(false))
            .await
            .unwrap()
            .lesson_menus,
        vec![ito_later]
    );
    assert_eq!(
        use_case
            .execute(Some(Sport::Tennis), Some(true))
            .await
            .unwrap()
            .lesson_menus,
        vec![ito_earlier, tanaka_tennis]
    );
    assert!(
        use_case
            .execute(Some(Sport::Swimming), None)
            .await
            .unwrap()
            .lesson_menus
            .is_empty()
    );
}
