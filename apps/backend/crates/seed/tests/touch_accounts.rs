//! 合成シード（種 42）を書いた DB で、触れる環境のログインの手がかり（`.factory-config/ship.yaml` の `touch.accounts`。
//! shipper が staging で実行する SQL）がすべて実在のアカウントをその役割で指すこと。本部の手がかりのトークンで、確定した
//! 予約を持つ合成のコーチをお休みにでき（admin-web の `getCoachLeavePreview` → `startCoachLeave` と同じユースケース）、
//! 予約がお休みの理由でキャンセルされ、本部の予約の詳細（`getBooking`）にお休みの記録が出ること。
//!
//! Issue #1480: 手がかりの本部が登録のない ID（モックのトークンはどの ID でも本部として通る）を指していたため、お休みの記録の
//! `created_by`（実在の本部のアカウントへの外部キー）を書けず、staging の `startCoachLeave` が毎回 500 になった。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p seed -- --include-ignored` で走らせる（テストごとに新しい DB を作ってマイグレーションを当てる）。

use std::{
    collections::BTreeSet,
    time::{Duration, SystemTime},
};

use application::{
    catalog::CatalogLessonMenuLookup,
    identity::account_id_uuid,
    in_memory::{FixedClock, RecordingAnalyticsSink, RecordingDomainEventPublisher},
    profile::{
        ProfileCoachAccountLookup, ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup,
    },
    scheduling::{
        GetBooking, PreviewCoachLeave, StartCoachLeave, StartCoachLeaveCommand, booking_id_uuid,
    },
};
use domain::{
    identity::AccountId,
    scheduling::{BookingCancelReason, BookingCanceller, BookingStatus, CoachLeaveNote},
};
use infra::{PgAccountRepository, PgBookingRepository, PgLessonMenuRepository};
use seed::{SeedPlan, Seeder};
use sqlx::{
    AssertSqlSafe, PgPool,
    types::{
        Uuid,
        time::{Date, OffsetDateTime},
    },
};

/// 2026-10-26T12:00:00+09:00（シードの計画のテストと同じ時刻）
const NOW: Duration = Duration::from_secs(1_792_983_600);

/// `.factory-config/ship.yaml` の `touch.accounts.command` の最後の引数（`psql -c` に渡す SQL）。
fn touch_accounts_sql() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../.factory-config/ship.yaml"
    );
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let ship: serde_yaml::Value = serde_yaml::from_str(&text).unwrap();
    let command = ship["touch"]["accounts"]["command"].as_sequence().unwrap();
    command.last().unwrap().as_str().unwrap().to_string()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn the_login_hints_point_at_seeded_accounts_and_the_headquarters_can_put_a_coach_on_leave(
    pool: PgPool,
) {
    let now = SystemTime::UNIX_EPOCH + NOW;
    Seeder::new(pool.clone(), RecordingAnalyticsSink::default())
        .run(&SeedPlan::generate(42, now))
        .await
        .unwrap();

    // 手がかりは JSON の配列 [{"label": ..., "value": "mock.<役割>.<アカウント ID>"}]
    // 出荷の設定に書いた固定の SQL（入力を混ぜない）を shipper と同じく実行する
    let sql = format!("SELECT ({})::text", touch_accounts_sql());
    let hints: String = sqlx::query_scalar(AssertSqlSafe(sql))
        .fetch_one(&pool)
        .await
        .unwrap();
    let hints: Vec<serde_json::Value> = serde_json::from_str(&hints).unwrap();
    let mut headquarters: Option<AccountId> = None;
    for hint in &hints {
        let token = hint["value"].as_str().unwrap();
        let mut parts = token.splitn(3, '.');
        let (mock, role, id) = (parts.next(), parts.next().unwrap(), parts.next().unwrap());
        assert_eq!(mock, Some("mock"), "{hint}");
        let found: Option<String> =
            sqlx::query_scalar("SELECT role FROM identity.accounts WHERE account_id = $1")
                .bind(Uuid::parse_str(id).unwrap())
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert_eq!(
            found.as_deref(),
            Some(role),
            "手がかりが実在のアカウントを指す: {hint}"
        );
        if role == "headquarters" {
            headquarters = Some(account_id_uuid::parse(id).unwrap());
        }
    }
    let headquarters = headquarters.unwrap_or_else(|| panic!("本部の手がかりがない: {hints:?}"));
    assert!(
        hints
            .iter()
            .any(|hint| hint["value"].as_str().unwrap().starts_with("mock.coach.")),
        "{hints:?}"
    );

    // 明後日以降に始まる確定した予約を持つコーチ（QA の「確定済みの予約を持つコーチ」）と、その枠の日（日本時間）
    let (coach, day, booking): (Uuid, Date, Uuid) = sqlx::query_as(
        "SELECT s.coach_id, (s.starts_at AT TIME ZONE 'Asia/Tokyo')::date, b.booking_id
         FROM scheduling.bookings b JOIN scheduling.slots s ON s.slot_id = b.slot_id
         WHERE b.status = 'confirmed' AND s.starts_at >= $1
         ORDER BY s.starts_at, b.booking_id LIMIT 1",
    )
    .bind(OffsetDateTime::from(
        now + Duration::from_secs(2 * 24 * 3600),
    ))
    .fetch_one(&pool)
    .await
    .unwrap();
    let coach = account_id_uuid::from_uuid(coach);
    let booking = booking_id_uuid::from_uuid(booking);

    let bookings = PgBookingRepository::new(pool.clone());
    let accounts = PgAccountRepository::new(pool.clone());
    let preview = PreviewCoachLeave::new(
        bookings.clone(),
        ProfileCoachAccountLookup::new(accounts.clone()),
        ProfileStudentDisplayNameLookup::new(accounts.clone()),
        CatalogLessonMenuLookup::new(PgLessonMenuRepository::new(pool.clone())),
        FixedClock(now),
    )
    .execute(coach, day, day)
    .await
    .unwrap();
    let expected: BTreeSet<_> = preview.targets.booking_ids();
    assert!(expected.contains(&booking), "{expected:?}");

    let leave = StartCoachLeave::new(
        bookings.clone(),
        ProfileCoachAccountLookup::new(accounts.clone()),
        RecordingDomainEventPublisher::default(),
        FixedClock(now),
    )
    .execute(StartCoachLeaveCommand {
        coach_id: coach,
        from: day,
        to: day,
        note: CoachLeaveNote::new("ご迷惑をおかけします。").unwrap(),
        requested_by: headquarters,
        expected_booking_ids: expected.clone(),
    })
    .await
    .unwrap();
    assert_eq!(leave.created_by(), headquarters);
    assert_eq!(leave.cancelled_bookings().len(), expected.len());

    let detail = GetBooking::new(
        bookings,
        ProfileCoachDisplayNameLookup::new(accounts.clone()),
        ProfileStudentDisplayNameLookup::new(accounts),
    )
    .execute(booking)
    .await
    .unwrap();
    assert_eq!(detail.booking.status(), BookingStatus::Cancelled);
    assert_eq!(
        detail.booking.cancel_reason(),
        Some(BookingCancelReason::CoachLeave)
    );
    assert_eq!(
        detail.booking.cancelled_by(),
        Some(BookingCanceller::Headquarters)
    );
    assert_eq!(detail.coach_leave, Some(leave));
}
