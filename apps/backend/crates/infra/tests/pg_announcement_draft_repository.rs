//! `PgAnnouncementDraftRepository`（`notification.announcement_drafts`）の検証。下書きの往復（書きかけの空の題・本文と
//! スポーツを絞ったもの）・最後に直した時刻の新しい順（同じ時刻は後に作ったものが先）・書き直し（作った本部と作った
//! 時刻は変えない）・削除（ないものは `false`）。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::notification::AnnouncementDraftRepository;
use domain::{
    catalog::Sport,
    identity::{AccountId, Role},
    notification::AnnouncementDraft,
};
use infra::PgAnnouncementDraftRepository;
use sqlx::PgPool;
use support::at;

fn draft(
    drafts: &PgAnnouncementDraftRepository,
    created_by: AccountId,
    title: &str,
    sport: Option<Sport>,
    hour: u64,
) -> AnnouncementDraft {
    AnnouncementDraft::new(drafts.next_id(), created_by, title, "", sport, at(hour)).unwrap()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn drafts_round_trip_most_recently_updated_first(pool: PgPool) {
    let drafts = PgAnnouncementDraftRepository::new(pool.clone());
    let headquarters = support::register(&pool, Role::Headquarters, vec![]).await;

    let old = draft(&drafts, headquarters, "", None, 1);
    let new = draft(
        &drafts,
        headquarters,
        "水泳の生徒へ",
        Some(Sport::Swimming),
        2,
    );
    let same_time = draft(&drafts, headquarters, "同じ時刻", None, 2);
    for draft in [&old, &new, &same_time] {
        drafts.insert(draft.clone()).await.unwrap();
    }

    assert_eq!(
        drafts.find_all().await.unwrap(),
        [same_time.clone(), new.clone(), old.clone()],
        "最後に直した時刻の新しい順。同じ時刻は後に作ったものが先"
    );
    assert_eq!(
        drafts.find(old.id()).await.unwrap(),
        Some(old.clone()),
        "空の題も往復する"
    );
    assert_eq!(drafts.find(drafts.next_id()).await.unwrap(), None);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_revision_keeps_who_created_it_and_moves_it_to_the_front(pool: PgPool) {
    let drafts = PgAnnouncementDraftRepository::new(pool.clone());
    let headquarters = support::register(&pool, Role::Headquarters, vec![]).await;
    let first = draft(&drafts, headquarters, "一つ目", None, 1);
    let second = draft(&drafts, headquarters, "二つ目", None, 2);
    drafts.insert(first.clone()).await.unwrap();
    drafts.insert(second.clone()).await.unwrap();

    let mut revised = first.clone();
    revised
        .revise("直した題", "1 行目\n2 行目", Some(Sport::Tennis), at(3))
        .unwrap();
    assert!(drafts.update(revised.clone()).await.unwrap());
    assert_eq!(
        drafts.find_all().await.unwrap(),
        [revised.clone(), second.clone()]
    );
    assert_eq!(revised.created_at(), at(1));

    let missing = draft(&drafts, headquarters, "ない", None, 4);
    assert!(
        !drafts.update(missing.clone()).await.unwrap(),
        "ない下書きは直さない"
    );
    assert_eq!(drafts.find(missing.id()).await.unwrap(), None);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn deleting_removes_only_that_draft(pool: PgPool) {
    let drafts = PgAnnouncementDraftRepository::new(pool.clone());
    let headquarters = support::register(&pool, Role::Headquarters, vec![]).await;
    let first = draft(&drafts, headquarters, "一つ目", None, 1);
    let second = draft(&drafts, headquarters, "二つ目", None, 2);
    drafts.insert(first.clone()).await.unwrap();
    drafts.insert(second.clone()).await.unwrap();

    assert!(drafts.delete(first.id()).await.unwrap());
    assert!(
        !drafts.delete(first.id()).await.unwrap(),
        "二度目は消すものがない"
    );
    assert_eq!(drafts.find_all().await.unwrap(), [second]);
}
