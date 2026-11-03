//! `PgAnnouncementDraftRepository`（`notification.announcement_drafts`）の検証。下書きの往復（書きかけの空の題・本文と
//! スポーツを絞ったもの）・最後に直した時刻の新しい順（同じ時刻は後に作ったものが先）・書き直し（作った本部と作った
//! 時刻は変えない）・削除（ないものは `false`）・一覧のユースケース（`ListAnnouncementDrafts`）が題の検索語・スポーツ・
//! 最後に直した日付（日本時間）の期間で絞ること。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::notification::{
    AnnouncementDraftRepository, ListAnnouncementDrafts, ListAnnouncementDraftsQuery,
};
use domain::{
    catalog::Sport,
    identity::{AccountId, Role},
    notification::{AnnouncementDraft, AnnouncementTitleQuery},
};
use infra::PgAnnouncementDraftRepository;
use sqlx::PgPool;
use support::at;
use time::{Date, Month};

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

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn listing_by_a_title_query_keeps_matching_drafts_in_the_same_order(pool: PgPool) {
    let drafts = PgAnnouncementDraftRepository::new(pool.clone());
    let headquarters = support::register(&pool, Role::Headquarters, vec![]).await;
    let maintenance = draft(&drafts, headquarters, "メンテナンスのお知らせ", None, 1);
    let app = draft(&drafts, headquarters, "App の更新", Some(Sport::Tennis), 2);
    let untitled = draft(&drafts, headquarters, "", None, 3);
    let night = draft(&drafts, headquarters, "夜間メンテナンス_100%", None, 4);
    for draft in [&maintenance, &app, &untitled, &night] {
        drafts.insert(draft.clone()).await.unwrap();
    }

    let list = ListAnnouncementDrafts::new(drafts.clone());
    let found = |q: &str| {
        let list = &list;
        let query = AnnouncementTitleQuery::new(q).unwrap();
        async move {
            list.execute(ListAnnouncementDraftsQuery {
                title: query,
                ..Default::default()
            })
            .await
            .unwrap()
        }
    };
    assert_eq!(
        found(" メンテナンス ").await,
        [night.clone(), maintenance.clone()],
        "題に部分一致するものだけ。最後に直した時刻の新しい順は変わらない"
    );
    assert_eq!(
        found("aPP").await,
        std::slice::from_ref(&app),
        "ASCII の英字は大文字・小文字を区別しない"
    );
    assert_eq!(
        found("_1").await,
        std::slice::from_ref(&night),
        "_ と % は文字のまま"
    );
    assert!(
        found("ＡＰＰ").await.is_empty(),
        "全角の英字は書いたとおりに比べる"
    );
    assert_eq!(
        found("\u{3000} ").await,
        [night, untitled, app, maintenance],
        "空白だけなら絞り込まない"
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn listing_by_a_sport_keeps_drafts_for_that_sport_in_the_same_order(pool: PgPool) {
    let drafts = PgAnnouncementDraftRepository::new(pool.clone());
    let headquarters = support::register(&pool, Role::Headquarters, vec![]).await;
    let tennis = draft(
        &drafts,
        headquarters,
        "テニスの大会",
        Some(Sport::Tennis),
        1,
    );
    let golf = draft(&drafts, headquarters, "ゴルフの大会", Some(Sport::Golf), 2);
    let everyone = draft(&drafts, headquarters, "テニスの大会と更新", None, 3);
    let result = draft(
        &drafts,
        headquarters,
        "テニスの大会の結果",
        Some(Sport::Tennis),
        4,
    );
    for draft in [&tennis, &golf, &everyone, &result] {
        drafts.insert(draft.clone()).await.unwrap();
    }

    let list = ListAnnouncementDrafts::new(drafts.clone());
    let found = |q: &str, sport: Option<Sport>| {
        let list = &list;
        let query = AnnouncementTitleQuery::new(q).unwrap();
        async move {
            list.execute(ListAnnouncementDraftsQuery {
                title: query,
                sport,
                ..Default::default()
            })
            .await
            .unwrap()
        }
    };
    assert_eq!(
        found("", Some(Sport::Tennis)).await,
        [result.clone(), tennis.clone()],
        "そのスポーツの生徒に向けた下書きだけ。生徒全員に向けた下書きは入らない。並べ方は変わらない"
    );
    assert_eq!(
        found("", Some(Sport::Golf)).await,
        std::slice::from_ref(&golf)
    );
    assert!(found("", Some(Sport::Swimming)).await.is_empty());
    assert_eq!(
        found("結果", Some(Sport::Tennis)).await,
        std::slice::from_ref(&result),
        "題と組み合わせられる"
    );
    assert!(
        found("更新", Some(Sport::Tennis)).await.is_empty(),
        "題が合っても生徒全員に向けた下書きは入らない"
    );
    assert_eq!(
        found("", None).await,
        [result, everyone, golf, tennis],
        "付けなければ今までどおりすべて"
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn listing_by_a_period_keeps_drafts_last_updated_in_it_in_japan_time(pool: PgPool) {
    let drafts = PgAnnouncementDraftRepository::new(pool.clone());
    let headquarters = support::register(&pool, Role::Headquarters, vec![]).await;
    // at(0) は 11/1 の 9 時（日本時間）。at(15) が 11/2 の 0 時、at(39) が 11/3 の 0 時
    let first = draft(&drafts, headquarters, "11/1 の朝", None, 0);
    let late = draft(&drafts, headquarters, "11/1 の夜", Some(Sport::Tennis), 14);
    let revised = draft(&drafts, headquarters, "直す下書き", None, 1);
    let last = draft(&drafts, headquarters, "11/3 の下書き", None, 39);
    for draft in [&first, &late, &revised, &last] {
        drafts.insert(draft.clone()).await.unwrap();
    }
    let mut revised = revised;
    revised.revise("直した下書き", "", None, at(15)).unwrap();
    assert!(drafts.update(revised.clone()).await.unwrap());

    let list = ListAnnouncementDrafts::new(drafts.clone());
    let nov = |day: u8| Date::from_calendar_date(2026, Month::November, day).unwrap();
    let found = |from: Option<u8>, to: Option<u8>, q: &str, sport: Option<Sport>| {
        let list = &list;
        let query = ListAnnouncementDraftsQuery {
            from: from.map(nov),
            to: to.map(nov),
            title: AnnouncementTitleQuery::new(q).unwrap(),
            sport,
        };
        async move { list.execute(query).await.unwrap() }
    };
    assert_eq!(
        found(Some(1), Some(1), "", None).await,
        [late.clone(), first.clone()],
        "その日の 0 時から翌日 0 時の手前まで（日本時間）。直した下書きは作った日付では入らない"
    );
    assert_eq!(
        found(Some(2), None, "", None).await,
        [last.clone(), revised.clone()],
        "from だけなら from から 7 日。最後に直した時刻で決まる"
    );
    assert_eq!(
        found(None, Some(2), "", None).await,
        [revised.clone(), late.clone(), first.clone()],
        "to だけなら to までの 7 日"
    );
    assert_eq!(
        found(Some(1), Some(3), "夜", Some(Sport::Tennis)).await,
        std::slice::from_ref(&late),
        "題・スポーツと組み合わせられる"
    );
    assert!(found(Some(4), None, "", None).await.is_empty());
    assert_eq!(
        found(None, None, "", None).await,
        [last, revised, late, first],
        "付けなければ今までどおりすべて"
    );
}
