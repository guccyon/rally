//! `PgAnnouncementRepository`（`notification.announcements`）の検証。送った記録の往復（全員とスポーツを絞ったもの）・
//! 送った時刻の新しい順（同じ時刻は後に残したものが先）・ID で引くこと・取り下げの往復（最初の取り下げを上書きしない）。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use std::collections::BTreeSet;

use application::notification::AnnouncementRepository;
use domain::{
    catalog::Sport,
    identity::{AccountId, Role},
    notification::{Announcement, AnnouncementBody, AnnouncementTitle, AnnouncementWithdrawal},
};
use infra::PgAnnouncementRepository;
use sqlx::PgPool;
use support::at;

fn announcement(
    announcements: &PgAnnouncementRepository,
    sender: AccountId,
    title: &str,
    sport: Option<Sport>,
    hour: u64,
) -> Announcement {
    Announcement::new(
        announcements.next_id(),
        sender,
        AnnouncementTitle::new(title).unwrap(),
        AnnouncementBody::new("1 行目\n2 行目").unwrap(),
        sport,
        3,
        at(hour),
    )
    .unwrap()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn announcements_round_trip_newest_first(pool: PgPool) {
    let announcements = PgAnnouncementRepository::new(pool.clone());
    let headquarters = support::register(&pool, Role::Headquarters, vec![]).await;

    let old = announcement(&announcements, headquarters, "全員へ", None, 1);
    let new = announcement(
        &announcements,
        headquarters,
        "水泳の生徒へ",
        Some(Sport::Swimming),
        2,
    );
    let same_time = announcement(&announcements, headquarters, "同じ時刻", None, 2);
    for announcement in [&old, &new, &same_time] {
        announcements.insert(announcement.clone()).await.unwrap();
    }

    assert_eq!(
        announcements.find_all().await.unwrap(),
        [same_time.clone(), new.clone(), old.clone()],
        "送った時刻の新しい順。同じ時刻は後に残したものが先"
    );

    let found = announcements
        .find_by_ids(&BTreeSet::from([
            new.id(),
            old.id(),
            announcements.next_id(),
        ]))
        .await
        .unwrap();
    assert_eq!(found.len(), 2, "ない ID は入れない");
    assert_eq!(found[&new.id()], new);
    assert_eq!(found[&old.id()], old);
    assert!(
        announcements
            .find_by_ids(&BTreeSet::new())
            .await
            .unwrap()
            .is_empty()
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_withdrawal_round_trips_and_the_first_one_stays(pool: PgPool) {
    let announcements = PgAnnouncementRepository::new(pool.clone());
    let headquarters = support::register(&pool, Role::Headquarters, vec![]).await;
    let other_headquarters = support::register(&pool, Role::Headquarters, vec![]).await;

    let sent = announcement(&announcements, headquarters, "誤って送った", None, 1);
    let kept = announcement(&announcements, headquarters, "そのまま", None, 2);
    for announcement in [&sent, &kept] {
        announcements.insert(announcement.clone()).await.unwrap();
    }
    assert_eq!(
        announcements.find(sent.id()).await.unwrap(),
        Some(sent.clone())
    );
    assert_eq!(
        announcements.find(announcements.next_id()).await.unwrap(),
        None
    );

    let mut withdrawn = sent.clone();
    withdrawn.withdraw(other_headquarters, at(3)).unwrap();
    announcements.update(withdrawn.clone()).await.unwrap();
    let found = announcements.find(sent.id()).await.unwrap().unwrap();
    assert_eq!(found, withdrawn);
    assert_eq!(
        found.withdrawal(),
        Some(&AnnouncementWithdrawal::new(other_headquarters, at(3)))
    );
    assert_eq!(
        announcements.find_all().await.unwrap(),
        [kept.clone(), withdrawn.clone()],
        "一覧には取り下げたものも載る"
    );
    assert_eq!(
        announcements
            .find_by_ids(&BTreeSet::from([sent.id()]))
            .await
            .unwrap()[&sent.id()],
        withdrawn
    );

    // 同時に取り下げた別の本部の書き込みは、最初の取り下げを上書きしない
    let mut late = sent.clone();
    late.withdraw(headquarters, at(4)).unwrap();
    announcements.update(late).await.unwrap();
    assert_eq!(
        announcements.find(sent.id()).await.unwrap(),
        Some(withdrawn)
    );
    assert_eq!(
        announcements.find(kept.id()).await.unwrap(),
        Some(kept),
        "ほかの記録は変えない"
    );
}
