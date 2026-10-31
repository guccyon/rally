//! `PgFavoriteCoachRepository`（`matching.favorite_coaches`）の検証。入れた順（新しい順・同じ時刻はコーチの ID の順）の
//! 往復・同じ組を 2 回入れても 1 件で最初の時刻のまま・外すのはその生徒のその組だけで何度外してもよいこと・
//! お気に入りのコーチの一覧のユースケースを Postgres の保存先で組み立てて表示名と評価が添わること。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::{
    in_memory::InMemoryLessonReviewRepository,
    matching::{FavoriteCoachRepository, ListFavoriteCoaches},
    review::ReviewCoachReviewSummaryLookup,
};
use domain::{catalog::Sport, identity::Role, matching::FavoriteCoach};
use infra::{PgAccountRepository, PgFavoriteCoachRepository};
use sqlx::PgPool;

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn favorites_round_trip_newest_first_and_once_per_pair(pool: PgPool) {
    let favorites = PgFavoriteCoachRepository::new(pool.clone());
    let (student, other) = (support::student(&pool).await, support::student(&pool).await);
    let (first, second, third) = (
        support::coach(&pool).await,
        support::coach(&pool).await,
        support::coach(&pool).await,
    );

    favorites
        .add(FavoriteCoach::restore(student, first, support::at(1)))
        .await
        .unwrap();
    favorites
        .add(FavoriteCoach::restore(student, second, support::at(2)))
        .await
        .unwrap();
    favorites
        .add(FavoriteCoach::restore(student, third, support::at(2)))
        .await
        .unwrap();
    // 同じ組をもう一度入れても 1 件で、最初の時刻のまま
    favorites
        .add(FavoriteCoach::restore(student, first, support::at(5)))
        .await
        .unwrap();
    favorites
        .add(FavoriteCoach::restore(other, first, support::at(3)))
        .await
        .unwrap();

    let (a, b) = if second < third {
        (second, third)
    } else {
        (third, second)
    };
    assert_eq!(
        favorites.find_by_student(student).await.unwrap(),
        [
            FavoriteCoach::restore(student, a, support::at(2)),
            FavoriteCoach::restore(student, b, support::at(2)),
            FavoriteCoach::restore(student, first, support::at(1)),
        ],
        "新しい順。同じ時刻はコーチの ID の順"
    );
    assert_eq!(
        favorites.find_by_student(other).await.unwrap(),
        [FavoriteCoach::restore(other, first, support::at(3))]
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn remove_takes_only_the_pair_and_is_idempotent(pool: PgPool) {
    let favorites = PgFavoriteCoachRepository::new(pool.clone());
    let (student, other) = (support::student(&pool).await, support::student(&pool).await);
    let (coach, kept) = (support::coach(&pool).await, support::coach(&pool).await);
    for favorite in [
        FavoriteCoach::restore(student, coach, support::at(1)),
        FavoriteCoach::restore(student, kept, support::at(1)),
        FavoriteCoach::restore(other, coach, support::at(1)),
    ] {
        favorites.add(favorite).await.unwrap();
    }

    favorites.remove(student, coach).await.unwrap();
    favorites.remove(student, coach).await.unwrap();

    assert_eq!(
        favorites.find_by_student(student).await.unwrap(),
        [FavoriteCoach::restore(student, kept, support::at(1))]
    );
    assert_eq!(
        favorites.find_by_student(other).await.unwrap(),
        [FavoriteCoach::restore(other, coach, support::at(1))],
        "ほかの生徒のお気に入りは変えない"
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn list_favorite_coaches_reads_display_names_from_postgres(pool: PgPool) {
    let favorites = PgFavoriteCoachRepository::new(pool.clone());
    let student = support::student(&pool).await;
    let coach = support::register_named(
        &pool,
        Role::Coach,
        vec![Sport::Soccer, Sport::Tennis],
        "たなか",
    )
    .await;
    favorites
        .add(FavoriteCoach::restore(student, coach, support::at(1)))
        .await
        .unwrap();

    let listings = ListFavoriteCoaches::new(
        favorites,
        PgAccountRepository::new(pool.clone()),
        ReviewCoachReviewSummaryLookup::new(InMemoryLessonReviewRepository::default()),
    )
    .execute(student)
    .await
    .unwrap();
    assert_eq!(listings.len(), 1);
    assert_eq!(listings[0].coach_id, coach);
    assert_eq!(listings[0].display_name.as_str(), "たなか");
    assert_eq!(
        listings[0].sports.iter().copied().collect::<Vec<_>>(),
        [Sport::Tennis, Sport::Soccer]
    );
    assert_eq!(listings[0].review_summary.review_count(), 0);
    assert_eq!(listings[0].favorited_at, support::at(1));
}
