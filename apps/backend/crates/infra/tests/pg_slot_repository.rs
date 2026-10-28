//! `PgSlotRepository`（`scheduling.slots`）の検証。重なる枠の拒否（並行な公開を含む）と、
//! 検索の絞り込み・ページング。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

use std::{
    num::NonZeroUsize,
    time::{Duration, SystemTime},
};

use application::{
    identity::{IdentityCoachIntroductionLookup, account_id_uuid},
    in_memory::{FixedClock, RecordingAnalyticsSink},
    scheduling::{
        PublishSlot, PublishSlotCommand, PublishSlotError, SearchSlots, SearchSlotsQuery,
        SlotRepository,
    },
};
use domain::{
    DomainError,
    identity::{AccountId, Sport},
    scheduling::{Slot, SlotStatus},
};
use infra::{PgAccountRepository, PgSlotRepository};
use sqlx::PgPool;

/// 2026-11-01T00:00:00Z
const BASE: Duration = Duration::from_secs(1_793_491_200);

fn at(hour: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + BASE + Duration::from_secs(hour * 3600)
}

/// コーチのアカウントと identity.coaches（slots.coach_id の外部キー）・identity.coach_sports の行を作る。
async fn coach(pool: &PgPool) -> AccountId {
    let id: sqlx::types::Uuid = sqlx::query_scalar(
        "INSERT INTO identity.accounts (role, email, display_name)
         VALUES ('coach', gen_random_uuid()::text || '@example.com', 'コーチ')
         RETURNING account_id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO identity.coaches (coach_id) VALUES ($1)")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    // 検索がコーチの紹介を引く（提供スポーツのないコーチは読めない）
    sqlx::query("INSERT INTO identity.coach_sports (coach_id, sport) VALUES ($1, 'tennis')")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    account_id_uuid::from_uuid(id)
}

fn publish_slot(
    pool: &PgPool,
) -> PublishSlot<PgSlotRepository, RecordingAnalyticsSink, FixedClock> {
    PublishSlot::new(
        PgSlotRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        FixedClock(at(0)),
    )
}

fn command(coach_id: AccountId, sport: Sport, from: u64, to: u64) -> PublishSlotCommand {
    PublishSlotCommand {
        coach_id,
        sport,
        starts_at: at(from),
        ends_at: at(to),
    }
}

fn query(limit: usize) -> SearchSlotsQuery {
    SearchSlotsQuery {
        sport: Sport::Tennis,
        coach_id: None,
        starts_after: None,
        starts_before: None,
        limit: NonZeroUsize::new(limit).unwrap(),
        cursor: None,
    }
}

async fn set_status(pool: &PgPool, slot: &Slot, status: &str) {
    sqlx::query("UPDATE scheduling.slots SET status = $1 WHERE slot_id = $2")
        .bind(status)
        .bind(application::scheduling::slot_id_uuid::to_uuid(slot.id()))
        .execute(pool)
        .await
        .unwrap();
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn published_slot_round_trips(pool: PgPool) {
    let coach_id = coach(&pool).await;
    let published = publish_slot(&pool)
        .execute(command(coach_id, Sport::Golf, 10, 11))
        .await
        .unwrap();

    let found = SearchSlots::new(
        PgSlotRepository::new(pool.clone()),
        IdentityCoachIntroductionLookup::new(PgAccountRepository::new(pool.clone())),
        FixedClock(at(0)),
    )
    .execute(SearchSlotsQuery {
        sport: Sport::Golf,
        ..query(20)
    })
    .await
    .unwrap();
    assert_eq!(found.items, vec![published]);
    assert_eq!(found.items[0].status(), SlotStatus::Open);
    assert_eq!(
        found.coaches[&coach_id].display_name.as_str(),
        "コーチ",
        "枠のコーチの紹介を identity から引く"
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn overlapping_slot_of_the_same_coach_is_rejected(pool: PgPool) {
    let coach_id = coach(&pool).await;
    let other_coach = coach(&pool).await;
    let use_case = publish_slot(&pool);
    let first = use_case
        .execute(command(coach_id, Sport::Tennis, 10, 12))
        .await
        .unwrap();

    for (from, to) in [(10, 12), (11, 13), (9, 11), (9, 13)] {
        let result = use_case
            .execute(command(coach_id, Sport::Soccer, from, to))
            .await;
        assert!(
            matches!(
                &result,
                Err(PublishSlotError::Invalid(DomainError::SlotOverlaps { existing }))
                    if *existing == first.id()
            ),
            "{from}-{to}: {result:?}"
        );
    }
    // 隣り合う枠・別のコーチの枠は公開できる
    use_case
        .execute(command(coach_id, Sport::Tennis, 12, 13))
        .await
        .unwrap();
    use_case
        .execute(command(other_coach, Sport::Tennis, 10, 12))
        .await
        .unwrap();
    // 予約済みの枠も時間を占め、閉じた枠は占めない
    set_status(&pool, &first, "booked").await;
    assert!(
        use_case
            .execute(command(coach_id, Sport::Tennis, 10, 11))
            .await
            .is_err()
    );
    set_status(&pool, &first, "closed").await;
    use_case
        .execute(command(coach_id, Sport::Tennis, 10, 11))
        .await
        .unwrap();

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM scheduling.slots")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 4);
    assert_eq!(use_case.analytics().events().len(), 4);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn concurrent_overlapping_publishes_save_only_one(pool: PgPool) {
    let coach_id = coach(&pool).await;
    for round in 0..10 {
        let hour = round * 10;
        let a = publish_slot(&pool);
        let b = publish_slot(&pool);
        let (ra, rb) = tokio::join!(
            a.execute(command(coach_id, Sport::Tennis, hour, hour + 2)),
            b.execute(command(coach_id, Sport::Tennis, hour + 1, hour + 3)),
        );
        assert_eq!(
            [ra.is_ok(), rb.is_ok()].iter().filter(|ok| **ok).count(),
            1,
            "round {round}: {ra:?} / {rb:?}"
        );
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM scheduling.slots")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 10);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn search_filters_open_slots_and_pages_in_start_order(pool: PgPool) {
    let coach_a = coach(&pool).await;
    let coach_b = coach(&pool).await;
    let use_case = publish_slot(&pool);
    let publish = async |coach_id, sport, hour| {
        use_case
            .execute(command(coach_id, sport, hour, hour + 1))
            .await
            .unwrap()
    };
    let past = publish(coach_a, Sport::Tennis, 1).await;
    let t13 = publish(coach_a, Sport::Tennis, 13).await;
    let t11 = publish(coach_a, Sport::Tennis, 11).await;
    let _soccer = publish(coach_a, Sport::Soccer, 12).await;
    let b11 = publish(coach_b, Sport::Tennis, 11).await;
    let t14 = publish(coach_b, Sport::Tennis, 14).await;
    let booked = publish(coach_a, Sport::Tennis, 15).await;
    set_status(&pool, &booked, "booked").await;

    // 現在時刻は 10 時（過去の枠は出ない）
    let search = SearchSlots::new(
        PgSlotRepository::new(pool.clone()),
        IdentityCoachIntroductionLookup::new(PgAccountRepository::new(pool.clone())),
        FixedClock(at(10)),
    );
    let ids = |slots: &[Slot]| slots.iter().map(Slot::id).collect::<Vec<_>>();

    // 同じ開始時刻はスロット ID の順
    let (first_11, second_11) = if t11.id() < b11.id() {
        (&t11, &b11)
    } else {
        (&b11, &t11)
    };
    let all = search.execute(query(20)).await.unwrap();
    assert_eq!(
        ids(&all.items),
        [first_11.id(), second_11.id(), t13.id(), t14.id()]
    );
    assert_eq!(all.next_cursor, None);

    // ページング
    let page1 = search.execute(query(3)).await.unwrap();
    assert_eq!(ids(&page1.items), [first_11.id(), second_11.id(), t13.id()]);
    let page2 = search
        .execute(SearchSlotsQuery {
            cursor: page1.next_cursor,
            ..query(3)
        })
        .await
        .unwrap();
    assert_eq!(ids(&page2.items), [t14.id()]);
    assert_eq!(page2.next_cursor, None);

    // コーチと期間で絞る
    let by_coach = search
        .execute(SearchSlotsQuery {
            coach_id: Some(coach_b),
            ..query(20)
        })
        .await
        .unwrap();
    assert_eq!(ids(&by_coach.items), [b11.id(), t14.id()]);
    let by_period = search
        .execute(SearchSlotsQuery {
            starts_after: Some(at(1)),
            starts_before: Some(at(13)),
            ..query(20)
        })
        .await
        .unwrap();
    assert_eq!(
        ids(&by_period.items),
        [past.id(), first_11.id(), second_11.id()]
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn search_of_the_repository_respects_the_limit(pool: PgPool) {
    let coach_id = coach(&pool).await;
    let use_case = publish_slot(&pool);
    for hour in 10..15 {
        use_case
            .execute(command(coach_id, Sport::Tennis, hour, hour + 1))
            .await
            .unwrap();
    }
    let found = PgSlotRepository::new(pool.clone())
        .search(&application::scheduling::SlotCriteria {
            sport: Sport::Tennis,
            coach_id: None,
            starts_after: at(0),
            starts_before: None,
            after: None,
            limit: 2,
        })
        .await
        .unwrap();
    assert_eq!(found.len(), 2);
}
