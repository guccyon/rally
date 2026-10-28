//! `PgEntitlementRepository`（`entitlement.*`）の検証。月額契約（プランつき）の開始と、暦月の通話の枠の消費・戻し・
//! プランの上限を超える回の拒否（standard の 3 回目・plus の 5 回目）・月をまたいだリセット（ADR 0005）。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use std::time::{Duration, SystemTime};

use application::{
    entitlement::{
        ConsumeLiveLessonQuota, ConsumeLiveLessonQuotaCommand, ConsumeLiveLessonQuotaError,
        EntitlementRepository, ReleaseLiveLessonQuota, ReleaseLiveLessonQuotaCommand,
        StartSubscription, StartSubscriptionCommand, StartSubscriptionError, entitlement_id_uuid,
    },
    in_memory::{FixedClock, RecordingAnalyticsSink, RecordingDomainEventPublisher},
    scheduling::booking_id_uuid,
};
use domain::{
    DomainError,
    entitlement::{ContractType, SubscriptionPlan, YearMonth},
    identity::{AccountId, Sport},
    scheduling::BookingId,
};
use infra::PgEntitlementRepository;
use sqlx::{PgPool, types::Uuid};

/// 2026-11-10T00:00:00Z（日本時間でも 11 月）
const NOV10: Duration = Duration::from_secs(1_794_268_800);
const DEC10: Duration = Duration::from_secs(1_796_860_800);

fn nov10() -> SystemTime {
    SystemTime::UNIX_EPOCH + NOV10
}

fn dec10() -> SystemTime {
    SystemTime::UNIX_EPOCH + DEC10
}

async fn subscribe(pool: &PgPool, student: AccountId) -> Result<(), StartSubscriptionError> {
    subscribe_to(pool, student, SubscriptionPlan::Standard).await
}

async fn subscribe_to(
    pool: &PgPool,
    student: AccountId,
    plan: SubscriptionPlan,
) -> Result<(), StartSubscriptionError> {
    StartSubscription::new(
        PgEntitlementRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        RecordingDomainEventPublisher::default(),
        FixedClock(nov10()),
    )
    .execute(StartSubscriptionCommand {
        student_id: student,
        sport: Sport::Tennis,
        plan,
    })
    .await
    .map(|_| ())
}

fn booking() -> BookingId {
    booking_id_uuid::from_uuid(Uuid::new_v4())
}

async fn consume(
    pool: &PgPool,
    student: AccountId,
    booking_id: BookingId,
    at: SystemTime,
) -> Result<(), ConsumeLiveLessonQuotaError> {
    ConsumeLiveLessonQuota::new(PgEntitlementRepository::new(pool.clone()))
        .execute(ConsumeLiveLessonQuotaCommand {
            student_id: student,
            booking_id,
            at,
        })
        .await
        .map(|_| ())
}

/// 受講権の暦月の枠の行（month, remaining_live_lessons, 使った予約の数）。
async fn quota_rows(pool: &PgPool) -> Vec<(String, i16, i64)> {
    sqlx::query_as(
        "SELECT to_char(q.month, 'YYYY-MM'), q.remaining_live_lessons,
                (SELECT count(*) FROM entitlement.live_lesson_quota_bookings b
                 WHERE b.entitlement_id = q.entitlement_id AND b.month = q.month)
         FROM entitlement.live_lesson_quotas q ORDER BY q.month",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn subscription_creates_a_monthly_entitlement_once(pool: PgPool) {
    let student = support::student(&pool).await;
    subscribe(&pool, student).await.unwrap();

    let entitlement = PgEntitlementRepository::new(pool.clone())
        .find_by_student(student)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(entitlement.contract_type(), ContractType::Monthly);
    assert_eq!(entitlement.sport(), Sport::Tennis);
    assert_eq!(entitlement.plan(), Some(SubscriptionPlan::Standard));
    let quota = entitlement.live_lesson_quota().unwrap();
    assert_eq!(quota.month(), YearMonth::new(2026, 11).unwrap());
    assert_eq!((quota.used(), quota.limit()), (0, 2));
    let plan: String = sqlx::query_scalar("SELECT plan FROM entitlement.subscriptions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(plan, "standard");
    assert_eq!(quota_rows(&pool).await, [("2026-11".to_string(), 2, 0)]);

    let result = subscribe(&pool, student).await;
    assert!(
        matches!(result, Err(StartSubscriptionError::AlreadySubscribed)),
        "{result:?}"
    );
    let other = support::student(&pool).await;
    assert_eq!(
        PgEntitlementRepository::new(pool.clone())
            .find_by_student(other)
            .await
            .unwrap(),
        None
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn quota_is_two_per_calendar_month_and_released_bookings_free_it(pool: PgPool) {
    let student = support::student(&pool).await;
    subscribe(&pool, student).await.unwrap();
    let (first, second, third) = (booking(), booking(), booking());

    consume(&pool, student, first, nov10()).await.unwrap();
    // 同じ予約で 2 回は使わない
    consume(&pool, student, first, nov10()).await.unwrap();
    consume(&pool, student, second, nov10()).await.unwrap();
    assert_eq!(quota_rows(&pool).await, [("2026-11".to_string(), 0, 2)]);
    let result = consume(&pool, student, third, nov10()).await;
    assert!(
        matches!(
            result,
            Err(ConsumeLiveLessonQuotaError::Invalid(
                DomainError::LiveLessonQuotaExceeded
            ))
        ),
        "{result:?}"
    );

    ReleaseLiveLessonQuota::new(PgEntitlementRepository::new(pool.clone()))
        .execute(ReleaseLiveLessonQuotaCommand {
            student_id: student,
            booking_id: second,
            at: nov10(),
        })
        .await
        .unwrap();
    assert_eq!(quota_rows(&pool).await, [("2026-11".to_string(), 1, 1)]);
    consume(&pool, student, third, nov10()).await.unwrap();

    // 翌月は 0 から数え、読むのは最後に記録した暦月の枠
    consume(&pool, student, booking(), dec10()).await.unwrap();
    assert_eq!(
        quota_rows(&pool).await,
        [("2026-11".to_string(), 0, 2), ("2026-12".to_string(), 1, 1)]
    );
    let entitlement = PgEntitlementRepository::new(pool.clone())
        .find_by_student(student)
        .await
        .unwrap()
        .unwrap();
    let quota = entitlement.live_lesson_quota().unwrap();
    assert_eq!(
        (quota.month(), quota.used()),
        (YearMonth::new(2026, 12).unwrap(), 1)
    );
    let entitlement_id: Uuid =
        sqlx::query_scalar("SELECT entitlement_id FROM entitlement.entitlements")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        entitlement_id,
        entitlement_id_uuid::to_uuid(entitlement.id())
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn plus_quota_is_four_per_calendar_month(pool: PgPool) {
    let student = support::student(&pool).await;
    subscribe_to(&pool, student, SubscriptionPlan::Plus)
        .await
        .unwrap();
    let plan: String = sqlx::query_scalar("SELECT plan FROM entitlement.subscriptions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(plan, "plus");
    assert_eq!(quota_rows(&pool).await, [("2026-11".to_string(), 4, 0)]);

    for _ in 0..4 {
        consume(&pool, student, booking(), nov10()).await.unwrap();
    }
    assert_eq!(quota_rows(&pool).await, [("2026-11".to_string(), 0, 4)]);
    // 5 回目は拒む（不変条件 1）
    let result = consume(&pool, student, booking(), nov10()).await;
    assert!(
        matches!(
            result,
            Err(ConsumeLiveLessonQuotaError::Invalid(
                DomainError::LiveLessonQuotaExceeded
            ))
        ),
        "{result:?}"
    );

    // 翌月はプランの上限から数える
    consume(&pool, student, booking(), dec10()).await.unwrap();
    let entitlement = PgEntitlementRepository::new(pool.clone())
        .find_by_student(student)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(entitlement.plan(), Some(SubscriptionPlan::Plus));
    let quota = entitlement.live_lesson_quota().unwrap();
    assert_eq!(
        (quota.month(), quota.used(), quota.remaining()),
        (YearMonth::new(2026, 12).unwrap(), 1, 3)
    );
    assert_eq!(
        quota_rows(&pool).await,
        [("2026-11".to_string(), 0, 4), ("2026-12".to_string(), 3, 1)]
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn subscriptions_saved_before_the_plan_column_are_read_as_standard(pool: PgPool) {
    // 列を足す前の契約の行（plan を書かない INSERT）は既定の standard として読む
    let student = support::student(&pool).await;
    let student_id = application::identity::account_id_uuid::to_uuid(student);
    let subscription_id: Uuid = sqlx::query_scalar(
        "INSERT INTO entitlement.subscriptions (student_id, sport) VALUES ($1, 'tennis')
         RETURNING subscription_id",
    )
    .bind(student_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let entitlement_id: Uuid = sqlx::query_scalar(
        "INSERT INTO entitlement.entitlements
             (student_id, contract_type, sport, subscription_id, unlimited_video_and_chat)
         VALUES ($1, 'subscription', 'tennis', $2, true) RETURNING entitlement_id",
    )
    .bind(student_id)
    .bind(subscription_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO entitlement.live_lesson_quotas (entitlement_id, month, remaining_live_lessons)
         VALUES ($1, '2026-11-01', 2)",
    )
    .bind(entitlement_id)
    .execute(&pool)
    .await
    .unwrap();

    let entitlement = PgEntitlementRepository::new(pool.clone())
        .find_by_student(student)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(entitlement.plan(), Some(SubscriptionPlan::Standard));
    assert_eq!(entitlement.live_lesson_quota().unwrap().limit(), 2);
}
