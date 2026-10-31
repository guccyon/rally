//! `PgSlotRepository`（`scheduling.slots`）の検証。重なる枠の拒否（並行な公開を含む）と、
//! 検索の絞り込み・ページング。コーチの枠の読み取り（`slots_of_coach`）と、それを読むコーチの自分の枠の一覧。
//! 枠に結びつけたレッスンメニュー（`lesson_menu_id`）の読み書きと、検索・一覧に添えるメニュー。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

use std::{
    num::NonZeroUsize,
    time::{Duration, SystemTime},
};

use application::{
    catalog::{CatalogLessonMenuLookup, LessonMenuRepository},
    identity::account_id_uuid,
    in_memory::{FixedClock, RecordingAnalyticsSink},
    profile::ProfileCoachIntroductionLookup,
    scheduling::{
        BookingRepository, ListCoachSlots, PublishSlot, PublishSlotCommand, PublishSlotError,
        SearchSlots, SearchSlotsQuery, SlotRepository,
    },
};
use domain::{
    DomainError,
    catalog::{
        LessonDuration, LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription,
        LessonMenuName,
    },
    identity::{AccountId, Sport},
    scheduling::{Booking, Slot, SlotCapacity, SlotStatus},
};
use infra::{PgAccountRepository, PgBookingRepository, PgLessonMenuRepository, PgSlotRepository};
use sqlx::PgPool;

mod support;

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

type PgLessonMenuLookup = CatalogLessonMenuLookup<PgLessonMenuRepository>;

fn lesson_menus(pool: &PgPool) -> PgLessonMenuLookup {
    CatalogLessonMenuLookup::new(PgLessonMenuRepository::new(pool.clone()))
}

fn publish_slot(
    pool: &PgPool,
) -> PublishSlot<PgSlotRepository, PgLessonMenuLookup, RecordingAnalyticsSink, FixedClock> {
    PublishSlot::new(
        PgSlotRepository::new(pool.clone()),
        lesson_menus(pool),
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
        capacity: SlotCapacity::ONE,
        lesson_menu_id: None,
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
        lesson_menus(&pool),
        ProfileCoachIntroductionLookup::new(PgAccountRepository::new(pool.clone())),
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
async fn capacity_and_remaining_seats_round_trip(pool: PgPool) {
    let coach_id = coach(&pool).await;
    let published = publish_slot(&pool)
        .execute(PublishSlotCommand {
            capacity: SlotCapacity::new(3).unwrap(),
            ..command(coach_id, Sport::Tennis, 10, 11)
        })
        .await
        .unwrap();
    let slots = PgSlotRepository::new(pool.clone());
    let read = || async {
        slots
            .slots_of_coach(coach_id)
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
    };
    assert_eq!(read().await, published);
    assert_eq!(read().await.capacity().value(), 3);
    assert_eq!(read().await.remaining_seats(), 3);

    // 席の数は予約の行から数える（未確定も確定も数え、キャンセルは数えない）
    let (first, second) = (support::student(&pool).await, support::student(&pool).await);
    let confirmed = support::confirmed_booking(&pool, first, &published).await;
    let bookings = PgBookingRepository::new(pool.clone());
    let id = bookings.next_id();
    bookings
        .reserve(published.id(), second, |slot, existing| {
            Booking::reserve(id, second, false, slot, existing, at(0))
        })
        .await
        .unwrap();
    let slot = read().await;
    assert_eq!((slot.seats().pending(), slot.seats().confirmed()), (1, 1));
    assert_eq!(slot.remaining_seats(), 1);
    assert_eq!(slot.status(), SlotStatus::Open);
    let found = SearchSlots::new(
        slots.clone(),
        lesson_menus(&pool),
        ProfileCoachIntroductionLookup::new(PgAccountRepository::new(pool.clone())),
        FixedClock(at(0)),
    )
    .execute(query(20))
    .await
    .unwrap();
    assert_eq!(found.items, vec![slot]);

    bookings
        .update(confirmed.id(), |booking, slot| {
            booking.cancel(first, slot, at(1)).map(|_| ())
        })
        .await
        .unwrap();
    assert_eq!(read().await.remaining_seats(), 2);
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
    // 予約済みの枠（確定した予約で埋まった枠）も時間を占め、閉じた枠は占めない
    support::confirmed_booking(&pool, support::student(&pool).await, &first).await;
    let result = use_case
        .execute(command(coach_id, Sport::Tennis, 10, 11))
        .await;
    assert!(
        matches!(
            &result,
            Err(PublishSlotError::Invalid(DomainError::SlotOverlaps { existing }))
                if *existing == first.id()
        ),
        "{result:?}"
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
    support::confirmed_booking(&pool, support::student(&pool).await, &booked).await;

    // 現在時刻は 10 時（過去の枠は出ない）
    let search = SearchSlots::new(
        PgSlotRepository::new(pool.clone()),
        lesson_menus(&pool),
        ProfileCoachIntroductionLookup::new(PgAccountRepository::new(pool.clone())),
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

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn slots_of_coach_reads_own_slots_of_any_status(pool: PgPool) {
    let slots = PgSlotRepository::new(pool.clone());
    let bookings = PgBookingRepository::new(pool.clone());
    let (coach_id, other_coach) = (coach(&pool).await, coach(&pool).await);
    let student = support::student(&pool).await;
    let publish = publish_slot(&pool);
    let mut published = Vec::new();
    for (from, to) in [(40, 41), (10, 11), (30, 31), (20, 21), (50, 51)] {
        published.push(
            publish
                .execute(command(coach_id, Sport::Tennis, from, to))
                .await
                .unwrap(),
        );
    }
    let [cancelled, open, confirmed, pending, closed] = published.try_into().unwrap();
    let others = publish
        .execute(command(other_coach, Sport::Tennis, 10, 11))
        .await
        .unwrap();
    set_status(&pool, &closed, "closed").await;
    support::confirmed_booking(&pool, student, &confirmed).await;
    let reserve = |slot: &Slot| {
        let id = bookings.next_id();
        bookings.reserve(slot.id(), student, move |slot, existing| {
            Booking::reserve(id, student, false, slot, existing, at(0))
        })
    };
    reserve(&pending).await.unwrap();
    reserve(&others).await.unwrap();
    let to_cancel = reserve(&cancelled).await.unwrap();
    bookings
        .update(to_cancel.id(), |booking, slot| {
            booking.cancel(student, slot, at(1)).map(|_| ())
        })
        .await
        .unwrap();

    let mut found = slots.slots_of_coach(coach_id).await.unwrap();
    found.sort_by_key(|slot| slot.starts_at());
    let statuses: Vec<_> = found
        .iter()
        .map(|slot| (slot.id(), slot.status()))
        .collect();
    assert_eq!(
        statuses,
        [
            (open.id(), SlotStatus::Open),
            (pending.id(), SlotStatus::Open),
            (confirmed.id(), SlotStatus::Booked),
            (cancelled.id(), SlotStatus::Open),
            (closed.id(), SlotStatus::Closed),
        ],
        "別のコーチの枠は読まない"
    );
    assert_eq!(found[0], open);

    // ユースケースを通すと、開始時刻の古い順で、有効な予約が入っているかが添わる
    let list = ListCoachSlots::new(slots.clone(), lesson_menus(&pool), bookings)
        .execute(coach_id)
        .await
        .unwrap();
    let listed: Vec<_> = list
        .items
        .iter()
        .map(|(slot, has_active_booking)| (slot.id(), *has_active_booking))
        .collect();
    assert_eq!(
        listed,
        [
            (open.id(), false),
            (pending.id(), true),
            (confirmed.id(), true),
            (cancelled.id(), false),
            (closed.id(), false),
        ]
    );

    let nobody = coach(&pool).await;
    assert!(slots.slots_of_coach(nobody).await.unwrap().is_empty());
}

/// コーチ `coach_id` のテニスのメニュー（1 回の時間は `minutes` 分）を保存する。
async fn menu(pool: &PgPool, id: u128, coach_id: AccountId, minutes: i64) -> LessonMenu {
    let menu = LessonMenu::restore(
        domain::catalog::LessonMenuId::from_u128(id),
        coach_id,
        LessonMenuContent {
            sport: Sport::Tennis,
            format: LessonFormat::Live(LessonDuration::from_minutes(minutes).unwrap()),
            name: LessonMenuName::new("フォア改善").unwrap(),
            description: LessonMenuDescription::default(),
        },
        true,
        at(0),
    );
    PgLessonMenuRepository::new(pool.clone())
        .insert(menu.clone())
        .await
        .unwrap();
    menu
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn lesson_menu_of_the_slot_round_trips_and_is_added_to_search_and_list(pool: PgPool) {
    let coach_id = coach(&pool).await;
    let student = support::student(&pool).await;
    let sixty = menu(&pool, 1, coach_id, 60).await;
    let forty_five = menu(&pool, 2, coach_id, 45).await;
    let publish = publish_slot(&pool);

    let bound = publish
        .execute(PublishSlotCommand {
            lesson_menu_id: Some(sixty.id()),
            ..command(coach_id, Sport::Tennis, 10, 11)
        })
        .await
        .unwrap();
    let plain = publish
        .execute(command(coach_id, Sport::Tennis, 12, 13))
        .await
        .unwrap();
    assert_eq!(bound.lesson_menu_id(), Some(sixty.id()));

    // 枠の長さ（60 分）とメニューの時間（45 分）が違えば保存しない
    let result = publish
        .execute(PublishSlotCommand {
            lesson_menu_id: Some(forty_five.id()),
            ..command(coach_id, Sport::Tennis, 14, 15)
        })
        .await;
    assert!(
        matches!(
            result,
            Err(PublishSlotError::Invalid(
                DomainError::SlotLengthMismatchesLessonMenu { minutes: 45 }
            ))
        ),
        "{result:?}"
    );

    // 保存した枠を読み戻すと、結びついたメニューの ID も戻る（メニューのない枠は NULL のまま）
    let slots = PgSlotRepository::new(pool.clone());
    let mut found = slots.slots_of_coach(coach_id).await.unwrap();
    found.sort_by_key(|slot| slot.starts_at());
    assert_eq!(found, vec![bound.clone(), plain.clone()]);
    assert_eq!(found[1].lesson_menu_id(), None);

    // 予約の保存先が読むスロットにも戻る（予約はメニューを引き継がない）
    let bookings = PgBookingRepository::new(pool.clone());
    let id = bookings.next_id();
    bookings
        .reserve(bound.id(), student, move |slot, existing| {
            Booking::reserve(id, student, false, slot, existing, at(0))
        })
        .await
        .unwrap();
    let of_student = bookings.bookings_of_student(student).await.unwrap();
    assert_eq!(of_student[0].1.lesson_menu_id(), Some(sixty.id()));

    // 検索と自分の枠の一覧に、結びついたメニューが添わる
    let search = SearchSlots::new(
        PgSlotRepository::new(pool.clone()),
        lesson_menus(&pool),
        ProfileCoachIntroductionLookup::new(PgAccountRepository::new(pool.clone())),
        FixedClock(at(0)),
    );
    let page = search.execute(query(20)).await.unwrap();
    assert_eq!(
        page.items.iter().map(Slot::id).collect::<Vec<_>>(),
        [bound.id(), plain.id()]
    );
    assert_eq!(
        page.lesson_menus.keys().copied().collect::<Vec<_>>(),
        [sixty.id()]
    );
    let list_coach_slots = ListCoachSlots::new(
        PgSlotRepository::new(pool.clone()),
        lesson_menus(&pool),
        PgBookingRepository::new(pool.clone()),
    );
    let list = list_coach_slots.execute(coach_id).await.unwrap();
    assert_eq!(list.lesson_menus[&sixty.id()], sixty);

    // メニューを非公開にすると、検索には添わず（生徒には見せない）、コーチの一覧には添わる
    let mut unpublished = sixty.clone();
    unpublished
        .revise(
            coach_id,
            &domain::identity::Coach::new([Sport::Tennis]).unwrap(),
            sixty.content().clone(),
            false,
        )
        .unwrap();
    PgLessonMenuRepository::new(pool.clone())
        .update(unpublished.clone())
        .await
        .unwrap();
    let page = search.execute(query(20)).await.unwrap();
    assert_eq!(page.items[0].lesson_menu_id(), Some(sixty.id()));
    assert!(page.lesson_menus.is_empty());
    let list = list_coach_slots.execute(coach_id).await.unwrap();
    assert_eq!(list.lesson_menus[&sixty.id()], unpublished);
}
