//! `PgNotificationSettingsRepository`（`notification.notification_settings`）の検証。変えたことのないアカウントは既定
//! （すべてオン）・変更の往復・ほかのアカウントに触れないこと・並行な変更が互いを上書きしないこと。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::notification::NotificationSettingsRepository;
use domain::notification::{NotificationKind, NotificationSettings};
use infra::PgNotificationSettingsRepository;
use sqlx::PgPool;

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn an_account_that_has_not_changed_anything_reads_every_kind_on(pool: PgPool) {
    let settings = PgNotificationSettingsRepository::new(pool.clone());
    let student = support::student(&pool).await;
    assert_eq!(
        settings.find(student).await.unwrap(),
        NotificationSettings::new(student)
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn changes_round_trip_per_account(pool: PgPool) {
    let settings = PgNotificationSettingsRepository::new(pool.clone());
    let (student, other) = (support::student(&pool).await, support::student(&pool).await);

    let updated = settings
        .update(student, |s| {
            s.set(NotificationKind::BookingConfirmed, false);
            s.set(NotificationKind::MessageReceived, false);
        })
        .await
        .unwrap();
    let expected = NotificationSettings::restore(
        student,
        [
            NotificationKind::BookingConfirmed,
            NotificationKind::MessageReceived,
        ],
    );
    assert_eq!(updated, expected);
    assert_eq!(settings.find(student).await.unwrap(), expected);
    assert_eq!(
        settings.find(other).await.unwrap(),
        NotificationSettings::new(other),
        "ほかのアカウントは既定のまま"
    );

    // オンに戻せる
    settings
        .update(student, |s| s.set(NotificationKind::BookingConfirmed, true))
        .await
        .unwrap();
    assert_eq!(
        settings.find(student).await.unwrap(),
        NotificationSettings::restore(student, [NotificationKind::MessageReceived])
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn concurrent_changes_of_different_kinds_are_both_kept(pool: PgPool) {
    let settings = PgNotificationSettingsRepository::new(pool.clone());
    let student = support::student(&pool).await;

    for _ in 0..5 {
        let (a, b) = tokio::join!(
            settings.update(student, |s| s
                .set(NotificationKind::BookingConfirmed, false)),
            settings.update(student, |s| s
                .set(NotificationKind::VideoReviewReturned, false)),
        );
        a.unwrap();
        b.unwrap();
        assert_eq!(
            settings.find(student).await.unwrap(),
            NotificationSettings::restore(
                student,
                [
                    NotificationKind::BookingConfirmed,
                    NotificationKind::VideoReviewReturned,
                ]
            ),
            "片方の変更がもう片方を上書きしない"
        );
        settings
            .update(student, |s| {
                for kind in NotificationKind::ALL {
                    s.set(kind, true);
                }
            })
            .await
            .unwrap();
    }
}
