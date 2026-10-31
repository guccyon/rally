//! `PgCoachAcceptanceRepository`（`scheduling.coach_acceptances`）の検証。止める・再開するの往復と、行のないコーチは
//! 受付中であること、並行な停止の直列化、停止中のコーチの枠を検索（`PgSlotRepository::search`）から除くこと、
//! 予約の保存先（`PgBookingRepository::find_coach_acceptance`）が同じ状態を読むこと。止めた・再開した記録
//! （`scheduling.coach_acceptance_changes`）が状態と同じトランザクションで残り、新しい順に読めること。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use std::collections::BTreeSet;

use application::scheduling::{
    BookingRepository, CoachAcceptanceRepository, SlotCriteria, SlotRepository,
    UpdateCoachAcceptanceError,
};
use domain::{
    catalog::Sport,
    identity::{AccountId, Role},
    scheduling::{CoachAcceptance, CoachAcceptanceChange, CoachAcceptanceStatus, SchedulingError},
};
use infra::{PgBookingRepository, PgCoachAcceptanceRepository, PgSlotRepository};
use sqlx::PgPool;
use support::{at, coach, open_slot, register, student};

fn criteria() -> SlotCriteria {
    SlotCriteria {
        sport: Sport::Tennis,
        coach_id: None,
        starts_after: at(0),
        starts_before: None,
        after: None,
        limit: 20,
    }
}

/// 受付を止める・再開する本部のアカウント。
async fn headquarters(pool: &PgPool) -> AccountId {
    register(pool, Role::Headquarters, vec![]).await
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn suspend_and_resume_round_trip(pool: PgPool) {
    let acceptances = PgCoachAcceptanceRepository::new(pool.clone());
    let bookings = PgBookingRepository::new(pool.clone());
    let (suspended, accepting) = (coach(&pool).await, coach(&pool).await);
    let hq = headquarters(&pool).await;

    // 行のないコーチは受付中
    assert_eq!(
        bookings.find_coach_acceptance(suspended).await.unwrap(),
        CoachAcceptance::accepting(suspended)
    );
    assert!(acceptances.suspended_coaches().await.unwrap().is_empty());

    let stored = acceptances
        .update(suspended, |a| a.suspend(hq, at(0)))
        .await
        .unwrap();
    assert_eq!(stored.status(), CoachAcceptanceStatus::Suspended);
    assert_eq!(
        acceptances.suspended_coaches().await.unwrap(),
        BTreeSet::from([suspended])
    );
    assert_eq!(
        bookings.find_coach_acceptance(suspended).await.unwrap(),
        stored
    );
    assert_eq!(
        bookings
            .find_coach_acceptance(accepting)
            .await
            .unwrap()
            .status(),
        CoachAcceptanceStatus::Accepting
    );

    // 判定はクロージャ（domain）。違反なら何も変えない
    let rejected = acceptances
        .update(suspended, |a| a.suspend(hq, at(0)))
        .await;
    assert!(matches!(
        rejected,
        Err(UpdateCoachAcceptanceError::Rejected(
            SchedulingError::CoachAlreadySuspended
        ))
    ));

    let resumed = acceptances
        .update(suspended, |a| a.resume(hq, at(1)))
        .await
        .unwrap();
    assert_eq!(resumed.status(), CoachAcceptanceStatus::Accepting);
    assert!(acceptances.suspended_coaches().await.unwrap().is_empty());
    assert_eq!(
        bookings.find_coach_acceptance(suspended).await.unwrap(),
        resumed
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn concurrent_suspensions_of_a_coach_without_a_row_let_only_one_through(pool: PgPool) {
    let coach_id = coach(&pool).await;
    let hq = headquarters(&pool).await;
    let acceptances = PgCoachAcceptanceRepository::new(pool.clone());
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let acceptances = acceptances.clone();
            tokio::spawn(
                async move { acceptances.update(coach_id, |a| a.suspend(hq, at(0))).await },
            )
        })
        .collect();
    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.unwrap());
    }
    let succeeded = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(succeeded, 1, "{results:?}");
    assert!(results.iter().all(|r| matches!(
        r,
        Ok(_)
            | Err(UpdateCoachAcceptanceError::Rejected(
                SchedulingError::CoachAlreadySuspended
            ))
    )));
    assert_eq!(
        acceptances.changes(coach_id).await.unwrap().len(),
        1,
        "通った停止の記録だけが残る"
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn changes_are_recorded_with_who_and_when_and_read_newest_first(pool: PgPool) {
    let (coach_id, other_coach) = (coach(&pool).await, coach(&pool).await);
    let (hq, other_hq) = (headquarters(&pool).await, headquarters(&pool).await);
    let acceptances = PgCoachAcceptanceRepository::new(pool.clone());

    // 一度も止めていないコーチは記録がない
    assert!(acceptances.changes(coach_id).await.unwrap().is_empty());

    acceptances
        .update(coach_id, |a| a.suspend(hq, at(1)))
        .await
        .unwrap();
    acceptances
        .update(other_coach, |a| a.suspend(hq, at(2)))
        .await
        .unwrap();
    acceptances
        .update(coach_id, |a| a.resume(other_hq, at(3)))
        .await
        .unwrap();
    // 同じ時刻なら後に保存した方が先
    acceptances
        .update(coach_id, |a| a.suspend(hq, at(3)))
        .await
        .unwrap();
    // 違反は記録しない
    acceptances
        .update(coach_id, |a| a.suspend(other_hq, at(4)))
        .await
        .unwrap_err();

    assert_eq!(
        acceptances.changes(coach_id).await.unwrap(),
        vec![
            CoachAcceptanceChange::restore(coach_id, CoachAcceptanceStatus::Suspended, hq, at(3)),
            CoachAcceptanceChange::restore(
                coach_id,
                CoachAcceptanceStatus::Accepting,
                other_hq,
                at(3)
            ),
            CoachAcceptanceChange::restore(coach_id, CoachAcceptanceStatus::Suspended, hq, at(1)),
        ]
    );
    assert_eq!(
        acceptances.changes(other_coach).await.unwrap(),
        vec![CoachAcceptanceChange::restore(
            other_coach,
            CoachAcceptanceStatus::Suspended,
            hq,
            at(2)
        )]
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_change_that_cannot_be_recorded_leaves_the_acceptance_as_it_was(pool: PgPool) {
    let coach_id = coach(&pool).await;
    // 本部でないアカウントの記録は外部キーで入らない。状態も同じトランザクションなので変わらない
    let not_headquarters = student(&pool).await;
    let acceptances = PgCoachAcceptanceRepository::new(pool.clone());
    let result = acceptances
        .update(coach_id, |a| a.suspend(not_headquarters, at(1)))
        .await;
    assert!(
        matches!(result, Err(UpdateCoachAcceptanceError::Unavailable(_))),
        "{result:?}"
    );
    assert!(acceptances.suspended_coaches().await.unwrap().is_empty());
    assert!(acceptances.changes(coach_id).await.unwrap().is_empty());
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn search_leaves_out_slots_of_suspended_coaches(pool: PgPool) {
    let (suspended, accepting) = (coach(&pool).await, coach(&pool).await);
    let hidden = open_slot(&pool, suspended, 10, 11).await;
    let shown = open_slot(&pool, accepting, 12, 13).await;
    let hq = headquarters(&pool).await;
    let acceptances = PgCoachAcceptanceRepository::new(pool.clone());
    acceptances
        .update(suspended, |a| a.suspend(hq, at(0)))
        .await
        .unwrap();

    let slots = PgSlotRepository::new(pool.clone());
    let found = slots
        .search(&SlotCriteria {
            limit: 1,
            ..criteria()
        })
        .await
        .unwrap();
    assert_eq!(found, vec![shown.clone()], "除いた後で件数を数える");
    let found = slots
        .search(&SlotCriteria {
            coach_id: Some(suspended),
            ..criteria()
        })
        .await
        .unwrap();
    assert!(found.is_empty());
    // 枠そのものは変えない（コーチの自分の枠の一覧には残る）
    assert_eq!(
        slots.slots_of_coach(suspended).await.unwrap(),
        vec![hidden.clone()]
    );

    acceptances
        .update(suspended, |a| a.resume(hq, at(1)))
        .await
        .unwrap();
    assert_eq!(
        slots.search(&criteria()).await.unwrap(),
        vec![hidden, shown]
    );
}
