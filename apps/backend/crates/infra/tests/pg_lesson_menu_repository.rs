//! `PgLessonMenuRepository`（`catalog.lesson_menus`）の検証。コーチがメニューを作って直し、コーチの一覧と生徒が見る
//! 公開中の一覧を読めること。メニューのスポーツはコーチの提供スポーツ（`PgAccountRepository` から読む）。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::{
    catalog::{
        CreateLessonMenu, CreateLessonMenuCommand, CreateLessonMenuError, LessonMenuRepository,
        ListCoachLessonMenus, ListMyLessonMenus, UpdateLessonMenu, UpdateLessonMenuCommand,
    },
    identity::IdentityCoachLookup,
    in_memory::FixedClock,
};
use domain::{
    DomainError,
    catalog::{
        LessonDuration, LessonMenu, LessonMenuContent, LessonMenuDescription, LessonMenuId,
        LessonMenuName,
    },
    identity::{AccountId, Role, Sport},
    lesson::LessonType,
};
use infra::{PgAccountRepository, PgLessonMenuRepository};
use sqlx::PgPool;
use support::at;

fn content(name: &str, duration: Option<i64>, is_published: bool) -> LessonMenuContent {
    LessonMenuContent {
        name: LessonMenuName::new(name).unwrap(),
        description: LessonMenuDescription::new("初心者向け").unwrap(),
        duration: duration.map(|minutes| LessonDuration::new(minutes).unwrap()),
        is_published,
    }
}

async fn create(
    pool: &PgPool,
    coach_id: AccountId,
    sport: Sport,
    lesson_type: LessonType,
    content: LessonMenuContent,
    hour: u64,
) -> Result<LessonMenu, CreateLessonMenuError> {
    CreateLessonMenu::new(
        PgLessonMenuRepository::new(pool.clone()),
        IdentityCoachLookup::new(PgAccountRepository::new(pool.clone())),
        FixedClock(at(hour)),
    )
    .execute(CreateLessonMenuCommand {
        coach_id,
        sport,
        lesson_type,
        content,
    })
    .await
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn creates_revises_and_reads_back_menus(pool: PgPool) {
    let coach = support::register(&pool, Role::Coach, vec![Sport::Tennis, Sport::Golf]).await;
    let menus = PgLessonMenuRepository::new(pool.clone());

    let live = create(
        &pool,
        coach,
        Sport::Tennis,
        LessonType::Live,
        content("フォーム改善", Some(60), true),
        1,
    )
    .await
    .unwrap();
    let golf = create(
        &pool,
        coach,
        Sport::Golf,
        LessonType::Async,
        content("スイング添削", None, true),
        2,
    )
    .await
    .unwrap();
    assert_eq!(menus.find(live.id()).await.unwrap(), Some(live.clone()));
    assert_eq!(menus.find(golf.id()).await.unwrap(), Some(golf.clone()));
    assert_eq!(menus.find(LessonMenuId::from_u128(99)).await.unwrap(), None);

    // コーチが直して非公開にする。スポーツ・種類・作った時刻は変わらない
    let revised = UpdateLessonMenu::new(menus.clone(), FixedClock(at(3)))
        .execute(UpdateLessonMenuCommand {
            account_id: coach,
            lesson_menu_id: live.id(),
            content: content("サーブ特訓", Some(90), false),
        })
        .await
        .unwrap();
    assert_eq!(menus.find(live.id()).await.unwrap(), Some(revised.clone()));
    assert_eq!(
        (revised.sport(), revised.lesson_type(), revised.created_at()),
        (Sport::Tennis, LessonType::Live, at(1))
    );
    assert_eq!(revised.updated_at(), at(3));

    let mine = ListMyLessonMenus::new(menus.clone())
        .execute(coach)
        .await
        .unwrap();
    assert_eq!(mine, [revised, golf.clone()]);
    let published = ListCoachLessonMenus::new(menus)
        .execute(coach, None)
        .await
        .unwrap();
    assert_eq!(published, [golf], "非公開は出ない");
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn rejects_sports_the_coach_does_not_offer_and_non_coaches(pool: PgPool) {
    let coach = support::coach(&pool).await;
    let student = support::student(&pool).await;

    let result = create(
        &pool,
        coach,
        Sport::Soccer,
        LessonType::Async,
        content("添削", None, true),
        1,
    )
    .await;
    assert!(
        matches!(
            result,
            Err(CreateLessonMenuError::Invalid(
                DomainError::SportNotOfferedByCoach {
                    sport: Sport::Soccer
                }
            ))
        ),
        "{result:?}"
    );
    let result = create(
        &pool,
        student,
        Sport::Tennis,
        LessonType::Async,
        content("添削", None, true),
        1,
    )
    .await;
    assert!(
        matches!(result, Err(CreateLessonMenuError::CoachNotFound)),
        "{result:?}"
    );
    assert!(
        PgLessonMenuRepository::new(pool)
            .find_by_coach(coach)
            .await
            .unwrap()
            .is_empty()
    );
}
