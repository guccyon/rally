//! `PgLessonMenuRepository`（`catalog.lesson_menus`）の検証。コーチのアカウントを `PgAccountRepository` で読んで
//! メニューを作り・直し・読み戻せること、公開中の一覧とスポーツの絞り込み、他のコーチのメニューを直せないこと、
//! 本部が見る横断の一覧（`ListLessonMenus`。コーチの表示名は `ProfileCoachDisplayNameLookup` で読む）の絞り込み（スポーツ・公開中か・コーチ）と並べ方。
//! 本部による非公開（`UnpublishLessonMenu`。時刻と理由を残す）と公開に戻す操作（`RepublishLessonMenu`）、そのあいだコーチが
//! 公開し直せないこと。非公開にした・公開に戻した記録（`catalog.lesson_menu_publication_changes`）を誰が・いつ・理由とともに
//! メニューと同じトランザクションで残し、新しい順に読めること（`ListLessonMenuPublicationChanges`）と、並行な非公開は 1 つだけ通ること。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::{
    catalog::{
        ChangeLessonMenuPublicationError, CreateLessonMenu, CreateLessonMenuCommand,
        LessonMenuRepository, ListCoachLessonMenus, ListLessonMenuPublicationChanges,
        ListLessonMenus, ListMyLessonMenus, RepublishLessonMenu, UnpublishLessonMenu,
        UpdateLessonMenu, UpdateLessonMenuCommand, UpdateLessonMenuError,
    },
    in_memory::FixedClock,
    profile::ProfileCoachDisplayNameLookup,
    scheduling::SchedulingLessonMenuRecentBookingCountLookup,
};
use domain::{
    DomainError,
    catalog::Sport,
    catalog::{
        HeadquartersUnpublish, HeadquartersUnpublishReason, LessonDuration, LessonFormat,
        LessonMenu, LessonMenuContent, LessonMenuDescription, LessonMenuId, LessonMenuName,
        LessonMenuPublicationAction, LessonMenuPublicationChange,
    },
    identity::{AccountId, Role},
    lesson::LessonType,
};
use infra::{PgAccountRepository, PgBookingRepository, PgLessonMenuRepository};
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
        SchedulingLessonMenuRecentBookingCountLookup::new(PgBookingRepository::new(pool.clone())),
        FixedClock(at(100)),
    );
    let list = use_case.execute(None, None, None).await.unwrap();
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
            .execute(Some(Sport::Soccer), None, None)
            .await
            .unwrap()
            .lesson_menus,
        vec![tanaka_soccer]
    );
    assert_eq!(
        use_case
            .execute(None, Some(false), None)
            .await
            .unwrap()
            .lesson_menus,
        vec![ito_later.clone()]
    );
    assert_eq!(
        use_case
            .execute(Some(Sport::Tennis), Some(true), None)
            .await
            .unwrap()
            .lesson_menus,
        vec![ito_earlier.clone(), tanaka_tennis.clone()]
    );
    assert!(
        use_case
            .execute(Some(Sport::Swimming), None, None)
            .await
            .unwrap()
            .lesson_menus
            .is_empty()
    );

    // コーチで絞ると、公開中も非公開もまとめて返し、sport・published と組み合わせられる
    assert_eq!(
        use_case
            .execute(None, None, Some(ito))
            .await
            .unwrap()
            .lesson_menus,
        vec![ito_earlier.clone(), ito_later.clone()]
    );
    assert_eq!(
        use_case
            .execute(None, Some(false), Some(ito))
            .await
            .unwrap()
            .lesson_menus,
        vec![ito_later]
    );
    let tanaka_tennis_only = use_case
        .execute(Some(Sport::Tennis), None, Some(tanaka))
        .await
        .unwrap();
    assert_eq!(tanaka_tennis_only.lesson_menus, vec![tanaka_tennis]);
    assert_eq!(
        tanaka_tennis_only
            .coaches
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        [tanaka]
    );
    // いないコーチは空の一覧
    let unknown = use_case
        .execute(None, None, Some(AccountId::from_u128(0xdead)))
        .await
        .unwrap();
    assert!(unknown.lesson_menus.is_empty());
    assert!(unknown.coaches.is_empty());
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_unpublishes_and_republishes_a_menu(pool: PgPool) {
    let coach = support::coach(&pool).await;
    let hq = support::register(&pool, Role::Headquarters, vec![]).await;
    let menu = create(
        &pool,
        coach,
        1,
        content(Sport::Tennis, live(30), "フォア"),
        true,
    )
    .await;
    let menus = PgLessonMenuRepository::new(pool.clone());
    let reason = HeadquartersUnpublishReason::new("料金の記載が誤っている").unwrap();

    // 本部が非公開にすると、時刻と理由が残り、生徒の公開中の一覧から消える
    let unpublish = UnpublishLessonMenu::new(menus.clone(), FixedClock(at(10)));
    let unpublished = unpublish
        .execute(menu.id(), hq, reason.clone())
        .await
        .unwrap();
    assert_eq!(
        menus.find(menu.id()).await.unwrap(),
        Some(unpublished.clone())
    );
    assert!(!unpublished.is_published());
    assert_eq!(
        unpublished.headquarters_unpublish(),
        Some(&HeadquartersUnpublish::new(reason.clone(), at(10)))
    );
    assert!(
        ListCoachLessonMenus::new(menus.clone())
            .execute(coach, None)
            .await
            .unwrap()
            .is_empty()
    );

    // 二度目は断り、最初の時刻と理由を残す
    let result = unpublish
        .execute(menu.id(), hq, HeadquartersUnpublishReason::default())
        .await;
    assert!(
        matches!(
            result,
            Err(ChangeLessonMenuPublicationError::Invalid(
                DomainError::LessonMenuAlreadyUnpublishedByHeadquarters
            ))
        ),
        "{result:?}"
    );

    // コーチは公開し直せないが、非公開のまま中身は直せる（本部が非公開にしたことは残る）
    let update = UpdateLessonMenu::new(menus.clone(), PgAccountRepository::new(pool.clone()));
    let result = update
        .execute(UpdateLessonMenuCommand {
            coach_id: coach,
            lesson_menu_id: menu.id(),
            content: content(Sport::Tennis, live(30), "フォア（直した）"),
            is_published: true,
        })
        .await;
    assert!(
        matches!(
            result,
            Err(UpdateLessonMenuError::Invalid(
                DomainError::LessonMenuUnpublishedByHeadquarters
            ))
        ),
        "{result:?}"
    );
    assert_eq!(
        menus.find(menu.id()).await.unwrap(),
        Some(unpublished.clone())
    );
    let fixed = update
        .execute(UpdateLessonMenuCommand {
            coach_id: coach,
            lesson_menu_id: menu.id(),
            content: content(Sport::Tennis, live(30), "フォア（直した）"),
            is_published: false,
        })
        .await
        .unwrap();
    assert_eq!(menus.find(menu.id()).await.unwrap(), Some(fixed.clone()));
    assert_eq!(
        fixed.headquarters_unpublish(),
        unpublished.headquarters_unpublish()
    );

    // 本部が公開に戻すと、公開中になり、本部が非公開にしたことは消える
    let republish = RepublishLessonMenu::new(menus.clone(), FixedClock(at(20)));
    let republished = republish.execute(menu.id(), hq).await.unwrap();
    assert_eq!(
        menus.find(menu.id()).await.unwrap(),
        Some(republished.clone())
    );
    assert!(republished.is_published());
    assert_eq!(republished.headquarters_unpublish(), None);
    assert_eq!(republished.name().as_str(), "フォア（直した）");
    assert_eq!(
        ListCoachLessonMenus::new(menus.clone())
            .execute(coach, None)
            .await
            .unwrap(),
        vec![republished]
    );

    // 本部が非公開にしていないメニューは公開に戻せない
    let result = republish.execute(menu.id(), hq).await;
    assert!(
        matches!(
            result,
            Err(ChangeLessonMenuPublicationError::Invalid(
                DomainError::LessonMenuNotUnpublishedByHeadquarters
            ))
        ),
        "{result:?}"
    );
}

/// 本部が非公開にした・公開に戻すメニューを、コーチを登録して作る。
async fn published_menu(pool: &PgPool, name: &str) -> LessonMenu {
    let coach = support::coach(pool).await;
    create(pool, coach, 1, content(Sport::Tennis, live(30), name), true).await
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn publication_changes_are_recorded_with_who_when_and_reason_and_read_newest_first(
    pool: PgPool,
) {
    let (menu, other_menu) = (
        published_menu(&pool, "フォア").await,
        published_menu(&pool, "バック").await,
    );
    let hq = support::register(&pool, Role::Headquarters, vec![]).await;
    let other_hq = support::register(&pool, Role::Headquarters, vec![]).await;
    let menus = PgLessonMenuRepository::new(pool.clone());
    let list = ListLessonMenuPublicationChanges::new(menus.clone());
    let reason = |text: &str| HeadquartersUnpublishReason::new(text).unwrap();

    // 本部が一度も非公開にしていないメニューは空、ないメニューは None
    assert_eq!(list.execute(menu.id()).await.unwrap(), Some(Vec::new()));
    assert_eq!(
        list.execute(LessonMenuId::from_u128(99)).await.unwrap(),
        None
    );

    UnpublishLessonMenu::new(menus.clone(), FixedClock(at(2)))
        .execute(menu.id(), hq, reason("誤った記載"))
        .await
        .unwrap();
    UnpublishLessonMenu::new(menus.clone(), FixedClock(at(2)))
        .execute(other_menu.id(), hq, reason("他のメニュー"))
        .await
        .unwrap();
    // 断った操作は記録しない
    RepublishLessonMenu::new(menus.clone(), FixedClock(at(3)))
        .execute(other_menu.id(), other_hq)
        .await
        .unwrap();
    RepublishLessonMenu::new(menus.clone(), FixedClock(at(3)))
        .execute(other_menu.id(), other_hq)
        .await
        .unwrap_err();
    RepublishLessonMenu::new(menus.clone(), FixedClock(at(4)))
        .execute(menu.id(), other_hq)
        .await
        .unwrap();
    // 同じ時刻なら後に保存した方が先
    UnpublishLessonMenu::new(menus.clone(), FixedClock(at(4)))
        .execute(menu.id(), hq, reason(""))
        .await
        .unwrap();

    assert_eq!(
        list.execute(menu.id()).await.unwrap().unwrap(),
        vec![
            LessonMenuPublicationChange::restore(
                menu.id(),
                LessonMenuPublicationAction::Unpublished { reason: reason("") },
                hq,
                at(4)
            ),
            LessonMenuPublicationChange::restore(
                menu.id(),
                LessonMenuPublicationAction::Republished,
                other_hq,
                at(4)
            ),
            LessonMenuPublicationChange::restore(
                menu.id(),
                LessonMenuPublicationAction::Unpublished {
                    reason: reason("誤った記載")
                },
                hq,
                at(2)
            ),
        ]
    );
    assert_eq!(
        menus.publication_changes(other_menu.id()).await.unwrap(),
        vec![
            LessonMenuPublicationChange::restore(
                other_menu.id(),
                LessonMenuPublicationAction::Republished,
                other_hq,
                at(3)
            ),
            LessonMenuPublicationChange::restore(
                other_menu.id(),
                LessonMenuPublicationAction::Unpublished {
                    reason: reason("他のメニュー")
                },
                hq,
                at(2)
            ),
        ]
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn concurrent_unpublishes_of_a_menu_let_only_one_through(pool: PgPool) {
    let menu = published_menu(&pool, "フォア").await;
    let hq = support::register(&pool, Role::Headquarters, vec![]).await;
    let menus = PgLessonMenuRepository::new(pool.clone());
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let menus = menus.clone();
            let id = menu.id();
            tokio::spawn(async move {
                UnpublishLessonMenu::new(menus, FixedClock(at(2)))
                    .execute(id, hq, HeadquartersUnpublishReason::default())
                    .await
            })
        })
        .collect();
    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.unwrap());
    }
    assert_eq!(
        results.iter().filter(|r| r.is_ok()).count(),
        1,
        "{results:?}"
    );
    assert!(results.iter().all(|r| matches!(
        r,
        Ok(_)
            | Err(ChangeLessonMenuPublicationError::Invalid(
                DomainError::LessonMenuAlreadyUnpublishedByHeadquarters
            ))
    )));
    assert_eq!(
        menus.publication_changes(menu.id()).await.unwrap().len(),
        1,
        "通った非公開の記録だけが残る"
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_change_that_cannot_be_recorded_leaves_the_menu_as_it_was(pool: PgPool) {
    let menu = published_menu(&pool, "フォア").await;
    // 本部でないアカウントの記録は外部キーで入らない。メニューも同じトランザクションなので変わらない
    let not_headquarters = support::student(&pool).await;
    let menus = PgLessonMenuRepository::new(pool.clone());
    let result = UnpublishLessonMenu::new(menus.clone(), FixedClock(at(2)))
        .execute(
            menu.id(),
            not_headquarters,
            HeadquartersUnpublishReason::default(),
        )
        .await;
    assert!(
        matches!(
            result,
            Err(ChangeLessonMenuPublicationError::Unavailable(_))
        ),
        "{result:?}"
    );
    assert_eq!(menus.find(menu.id()).await.unwrap(), Some(menu.clone()));
    assert!(
        menus
            .publication_changes(menu.id())
            .await
            .unwrap()
            .is_empty()
    );

    // ないメニューは何も書かない
    let result = UnpublishLessonMenu::new(menus.clone(), FixedClock(at(2)))
        .execute(
            LessonMenuId::from_u128(99),
            not_headquarters,
            HeadquartersUnpublishReason::default(),
        )
        .await;
    assert!(
        matches!(
            result,
            Err(ChangeLessonMenuPublicationError::LessonMenuNotFound)
        ),
        "{result:?}"
    );
}
