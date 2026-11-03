//! `PgAnnouncementRepository`（`notification.announcements`）の検証。送った記録の往復（全員とスポーツを絞ったもの）・
//! 送った時刻の新しい順（同じ時刻は後に残したものが先）・送った時刻の期間で引くこと（開始を含み終了を含まない。取り下げた
//! ものも含む）・ID で引くこと・取り下げの往復（最初の取り下げを上書きしない）・一覧のユースケース（`ListAnnouncements`）が
//! 題の検索語で絞ること（期間と組み合わせられる）・対象のスポーツで絞ること（生徒全員に送ったものは入らない。期間・題と
//! 組み合わせられる）。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use std::collections::BTreeSet;

use application::notification::{
    AnnouncementRepository, ListAnnouncements, ListAnnouncementsQuery,
};
use domain::{
    catalog::Sport,
    identity::{AccountId, Role},
    notification::{
        Announcement, AnnouncementBody, AnnouncementTitle, AnnouncementTitleQuery,
        AnnouncementWithdrawal,
    },
};
use infra::{PgAnnouncementRepository, PgNotificationRepository};
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

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn announcements_sent_in_a_period_newest_first(pool: PgPool) {
    let announcements = PgAnnouncementRepository::new(pool.clone());
    let headquarters = support::register(&pool, Role::Headquarters, vec![]).await;

    let before = announcement(&announcements, headquarters, "前", None, 1);
    let first = announcement(&announcements, headquarters, "始まり", None, 2);
    let same_time = announcement(&announcements, headquarters, "同じ時刻", None, 2);
    let mut withdrawn = announcement(&announcements, headquarters, "取り下げた", None, 3);
    let end = announcement(&announcements, headquarters, "終わり", None, 4);
    for announcement in [&before, &first, &same_time, &withdrawn, &end] {
        announcements.insert(announcement.clone()).await.unwrap();
    }
    withdrawn.withdraw(headquarters, at(5)).unwrap();
    announcements.update(withdrawn.clone()).await.unwrap();

    assert_eq!(
        announcements.find_sent_between(at(2), at(4)).await.unwrap(),
        [withdrawn, same_time, first],
        "開始を含み終了を含まない。新しい順（同じ時刻は後に残したものが先）で、取り下げたものも含む"
    );
    assert!(
        announcements
            .find_sent_between(at(5), at(6))
            .await
            .unwrap()
            .is_empty()
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn listing_by_a_title_query_keeps_matching_announcements_in_the_same_order(pool: PgPool) {
    let announcements = PgAnnouncementRepository::new(pool.clone());
    let headquarters = support::register(&pool, Role::Headquarters, vec![]).await;
    // 1 時・2 時（UTC）は日本時間の 11/1、16 時は 11/2
    let maintenance = announcement(
        &announcements,
        headquarters,
        "メンテナンスのお知らせ",
        None,
        1,
    );
    let app = announcement(
        &announcements,
        headquarters,
        "App の更新",
        Some(Sport::Tennis),
        2,
    );
    let mut night = announcement(
        &announcements,
        headquarters,
        "夜間メンテナンス_100%",
        None,
        16,
    );
    for announcement in [&maintenance, &app, &night] {
        announcements.insert(announcement.clone()).await.unwrap();
    }
    night.withdraw(headquarters, at(17)).unwrap();
    announcements.update(night.clone()).await.unwrap();

    let list = ListAnnouncements::new(
        announcements.clone(),
        PgNotificationRepository::new(pool.clone()),
    );
    let found = |q: &str, from: Option<time::Date>| {
        let list = &list;
        let query = ListAnnouncementsQuery {
            from,
            to: from,
            title: AnnouncementTitleQuery::new(q).unwrap(),
            sport: None,
        };
        async move {
            list.execute(query)
                .await
                .unwrap()
                .into_iter()
                .map(|listed| listed.announcement)
                .collect::<Vec<_>>()
        }
    };
    let nov1 = time::Date::from_calendar_date(2026, time::Month::November, 1).unwrap();
    assert_eq!(
        found(" メンテナンス ", None).await,
        [night.clone(), maintenance.clone()],
        "題に部分一致するものだけ。新しい順は変わらず、取り下げたものも載る"
    );
    assert_eq!(
        found("aPP", None).await,
        std::slice::from_ref(&app),
        "ASCII の英字は大文字・小文字を区別しない"
    );
    assert_eq!(
        found("_1", None).await,
        std::slice::from_ref(&night),
        "_ と % は文字のまま"
    );
    assert!(
        found("ＡＰＰ", None).await.is_empty(),
        "全角の英字は書いたとおりに比べる"
    );
    assert_eq!(
        found("メンテナンス", Some(nov1)).await,
        std::slice::from_ref(&maintenance),
        "期間（日本時間の 11/1）と組み合わせられる"
    );
    assert_eq!(
        found("\u{3000} ", None).await,
        [night, app, maintenance],
        "空白だけなら絞り込まない"
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn listing_by_a_sport_keeps_announcements_sent_to_that_sport_in_the_same_order(pool: PgPool) {
    let announcements = PgAnnouncementRepository::new(pool.clone());
    let headquarters = support::register(&pool, Role::Headquarters, vec![]).await;
    // 1 時・2 時・3 時（UTC）は日本時間の 11/1、16 時・17 時は 11/2
    let everyone = announcement(
        &announcements,
        headquarters,
        "テニスとゴルフの大会",
        None,
        1,
    );
    let tennis = announcement(
        &announcements,
        headquarters,
        "テニスの大会",
        Some(Sport::Tennis),
        2,
    );
    let golf = announcement(
        &announcements,
        headquarters,
        "ゴルフの大会",
        Some(Sport::Golf),
        3,
    );
    let mut tennis_withdrawn = announcement(
        &announcements,
        headquarters,
        "テニスの大会（再）",
        Some(Sport::Tennis),
        16,
    );
    let tennis_result = announcement(
        &announcements,
        headquarters,
        "テニスの結果",
        Some(Sport::Tennis),
        17,
    );
    for announcement in [&everyone, &tennis, &golf, &tennis_withdrawn, &tennis_result] {
        announcements.insert(announcement.clone()).await.unwrap();
    }
    tennis_withdrawn.withdraw(headquarters, at(18)).unwrap();
    announcements
        .update(tennis_withdrawn.clone())
        .await
        .unwrap();

    let list = ListAnnouncements::new(
        announcements.clone(),
        PgNotificationRepository::new(pool.clone()),
    );
    let found = |query: ListAnnouncementsQuery| {
        let list = &list;
        async move {
            list.execute(query)
                .await
                .unwrap()
                .into_iter()
                .map(|listed| listed.announcement)
                .collect::<Vec<_>>()
        }
    };
    let nov1 = time::Date::from_calendar_date(2026, time::Month::November, 1).unwrap();
    let nov2 = time::Date::from_calendar_date(2026, time::Month::November, 2).unwrap();
    assert_eq!(
        found(ListAnnouncementsQuery {
            sport: Some(Sport::Tennis),
            ..Default::default()
        })
        .await,
        [
            tennis_result.clone(),
            tennis_withdrawn.clone(),
            tennis.clone()
        ],
        "テニスの生徒に向けて送ったものだけ。新しい順は変わらず、取り下げたものも載る。生徒全員に送ったものは入らない"
    );
    assert_eq!(
        found(ListAnnouncementsQuery {
            sport: Some(Sport::Golf),
            ..Default::default()
        })
        .await,
        std::slice::from_ref(&golf)
    );
    assert!(
        found(ListAnnouncementsQuery {
            sport: Some(Sport::Swimming),
            ..Default::default()
        })
        .await
        .is_empty(),
        "そのスポーツに送ったものがなければ空の一覧"
    );
    assert_eq!(
        found(ListAnnouncementsQuery {
            from: Some(nov2),
            to: Some(nov2),
            title: AnnouncementTitleQuery::new("大会").unwrap(),
            sport: Some(Sport::Tennis),
        })
        .await,
        std::slice::from_ref(&tennis_withdrawn),
        "期間（日本時間の 11/2）・題と組み合わせられる"
    );
    assert!(
        found(ListAnnouncementsQuery {
            from: Some(nov1),
            to: Some(nov1),
            title: AnnouncementTitleQuery::new("ゴルフ").unwrap(),
            sport: Some(Sport::Tennis),
        })
        .await
        .is_empty(),
        "題が一致しても生徒全員に送ったもの・ほかのスポーツに送ったものは入らない"
    );
    assert_eq!(
        found(ListAnnouncementsQuery::default()).await,
        [tennis_result, tennis_withdrawn, golf, tennis, everyone],
        "付けなければ今までどおりすべて"
    );
}
