//! `PgLessonMenuRepository`（`catalog.lesson_menus`）の検証。コーチのアカウントを `PgAccountRepository` で読んで
//! メニューを作り・直し・読み戻せること、公開中の一覧とスポーツの絞り込み、他のコーチのメニューを直せないこと。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::{
    catalog::{
        CreateLessonMenu, CreateLessonMenuCommand, LessonMenuRepository, ListCoachLessonMenus,
        ListMyLessonMenus, UpdateLessonMenu, UpdateLessonMenuCommand, UpdateLessonMenuError,
    },
    in_memory::FixedClock,
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
