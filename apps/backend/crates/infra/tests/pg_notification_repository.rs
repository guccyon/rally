//! `PgNotificationRepository`（`notification.notifications`）の検証。プッシュ通知の変種ごとの往復・受け取る人ごとの
//! 新しい順・同じプッシュ通知を 2 件にしないこと・すべて既読（最初に読んだ時刻のまま、後に作ったものは読まない）。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::notification::NotificationRepository;
use domain::{
    async_lesson::{VideoId, VideoReviewId},
    identity::AccountId,
    messaging::{MessageId, MessageSender, MessageThreadId},
    notification::{Notification, PushNotification},
    scheduling::BookingId,
};
use infra::PgNotificationRepository;
use sqlx::PgPool;
use support::at;

/// `student` と `coach` のあいだの、変種ごとのプッシュ通知（予約の確定・新しい予約・添削の返却・メッセージ 2 通）。
fn pushes(student: AccountId, coach: AccountId) -> [PushNotification; 5] {
    [
        PushNotification::BookingConfirmed {
            student_id: student,
            coach_id: coach,
            booking_id: BookingId::from_u128(1),
        },
        PushNotification::BookingReceived {
            student_id: student,
            coach_id: coach,
            booking_id: BookingId::from_u128(1),
        },
        PushNotification::VideoReviewReturned {
            student_id: student,
            coach_id: coach,
            video_id: VideoId::from_u128(2),
            review_id: VideoReviewId::from_u128(3),
        },
        PushNotification::MessageReceived {
            student_id: student,
            coach_id: coach,
            thread_id: MessageThreadId::from_u128(4),
            message_id: MessageId::from_u128(5),
            sender: MessageSender::Coach,
        },
        PushNotification::MessageReceived {
            student_id: student,
            coach_id: coach,
            thread_id: MessageThreadId::from_u128(4),
            message_id: MessageId::from_u128(6),
            sender: MessageSender::Student,
        },
    ]
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn every_kind_of_push_round_trips_to_its_recipient_newest_first(pool: PgPool) {
    let notifications = PgNotificationRepository::new(pool.clone());
    let (student, coach) = (support::student(&pool).await, support::coach(&pool).await);
    let [confirmed, received, returned, from_coach, from_student] = pushes(student, coach);

    let saved: Vec<Notification> = [
        (confirmed, 1),
        (received, 1),
        (returned, 2),
        (from_coach, 3),
        (from_student, 3),
    ]
    .into_iter()
    .map(|(push, hour)| Notification::new(notifications.next_id(), push, at(hour)))
    .collect();
    for notification in &saved {
        notifications.insert(*notification).await.unwrap();
    }

    assert_eq!(
        notifications.find_by_recipient(student).await.unwrap(),
        [saved[3], saved[2], saved[0]],
        "生徒には予約の確定・添削の返却・コーチからのメッセージ。新しい順"
    );
    assert_eq!(
        notifications.find_by_recipient(coach).await.unwrap(),
        [saved[4], saved[1]],
        "コーチには新しい予約・生徒からのメッセージ"
    );
    assert!(
        notifications
            .find_by_recipient(support::student(&pool).await)
            .await
            .unwrap()
            .is_empty()
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn the_same_push_is_kept_once_and_ties_put_the_later_first(pool: PgPool) {
    let notifications = PgNotificationRepository::new(pool.clone());
    let (student, coach) = (support::student(&pool).await, support::coach(&pool).await);
    let [confirmed, _, returned, ..] = pushes(student, coach);

    let first = Notification::new(notifications.next_id(), confirmed, at(1));
    notifications.insert(first).await.unwrap();
    // 同じプッシュ通知は別の ID・時刻でも足さない（障害にもしない）
    notifications
        .insert(Notification::new(notifications.next_id(), confirmed, at(5)))
        .await
        .unwrap();
    let later = Notification::new(notifications.next_id(), returned, at(1));
    notifications.insert(later).await.unwrap();

    assert_eq!(
        notifications.find_by_recipient(student).await.unwrap(),
        [later, first],
        "同じ時刻は後に残したものが先"
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn marking_all_read_keeps_the_first_time_and_skips_later_ones(pool: PgPool) {
    let notifications = PgNotificationRepository::new(pool.clone());
    let (student, coach) = (support::student(&pool).await, support::coach(&pool).await);
    let [confirmed, received, returned, from_coach, _] = pushes(student, coach);
    for (push, hour) in [
        (confirmed, 1),
        (received, 1),
        (returned, 2),
        (from_coach, 9),
    ] {
        notifications
            .insert(Notification::new(notifications.next_id(), push, at(hour)))
            .await
            .unwrap();
    }

    notifications.mark_all_read(student, at(5)).await.unwrap();
    let read_at = |list: Vec<Notification>| {
        list.iter()
            .map(|n| (n.push(), n.read_at()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        read_at(notifications.find_by_recipient(student).await.unwrap()),
        [
            (from_coach, None),
            (returned, Some(at(5))),
            (confirmed, Some(at(5)))
        ],
        "既読にした時刻より後に作ったものは読まない"
    );
    assert_eq!(
        read_at(notifications.find_by_recipient(coach).await.unwrap()),
        [(received, None)],
        "ほかの人のお知らせは変えない"
    );

    notifications.mark_all_read(student, at(10)).await.unwrap();
    assert_eq!(
        read_at(notifications.find_by_recipient(student).await.unwrap()),
        [
            (from_coach, Some(at(10))),
            (returned, Some(at(5))),
            (confirmed, Some(at(5)))
        ],
        "読んだものは最初に読んだ時刻のまま"
    );
}
