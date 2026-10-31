//! `PgNotificationRepository`（`notification.notifications`）の検証。プッシュ通知の変種ごとの往復・受け取る人ごとの
//! 新しい順・同じプッシュ通知（同じ動画の添削の依頼を含む）を 2 件にしないこと・すべて既読（最初に読んだ時刻のまま、
//! 後に作ったものは読まない。生徒とコーチのどちらも自分の分だけ）・本部からのお知らせ（コーチを持たず、送った記録を指す）の往復。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::notification::{AnnouncementRepository, NotificationRepository};
use domain::{
    async_lesson::{VideoId, VideoReviewId},
    identity::{AccountId, Role},
    messaging::{MessageId, MessageSender, MessageThreadId},
    notification::{
        Announcement, AnnouncementBody, AnnouncementTitle, Notification, PushNotification,
    },
    scheduling::BookingId,
};
use infra::{PgAnnouncementRepository, PgNotificationRepository};
use sqlx::PgPool;
use support::at;

/// `student` と `coach` のあいだの、変種ごとのプッシュ通知（予約の確定・新しい予約・添削の返却・メッセージ 2 通・
/// 添削の返却と同じ動画の添削の依頼）。
fn pushes(student: AccountId, coach: AccountId) -> [PushNotification; 6] {
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
        PushNotification::VideoReviewRequested {
            student_id: student,
            coach_id: coach,
            video_id: VideoId::from_u128(2),
        },
    ]
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn every_kind_of_push_round_trips_to_its_recipient_newest_first(pool: PgPool) {
    let notifications = PgNotificationRepository::new(pool.clone());
    let (student, coach) = (support::student(&pool).await, support::coach(&pool).await);
    let [
        confirmed,
        received,
        returned,
        from_coach,
        from_student,
        requested,
    ] = pushes(student, coach);

    let saved: Vec<Notification> = [
        (confirmed, 1),
        (received, 1),
        (returned, 2),
        (from_coach, 3),
        (from_student, 3),
        (requested, 2),
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
        [saved[4], saved[5], saved[1]],
        "コーチには新しい予約・添削の依頼・生徒からのメッセージ"
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
async fn the_same_review_request_is_kept_once_for_the_coach(pool: PgPool) {
    let notifications = PgNotificationRepository::new(pool.clone());
    let (student, coach) = (support::student(&pool).await, support::coach(&pool).await);
    let [.., requested] = pushes(student, coach);

    let first = Notification::new(notifications.next_id(), requested, at(1));
    notifications.insert(first).await.unwrap();
    notifications
        .insert(Notification::new(notifications.next_id(), requested, at(5)))
        .await
        .unwrap();
    let other_video = PushNotification::VideoReviewRequested {
        student_id: student,
        coach_id: coach,
        video_id: VideoId::from_u128(7),
    };
    let other = Notification::new(notifications.next_id(), other_video, at(2));
    notifications.insert(other).await.unwrap();

    assert_eq!(
        notifications.find_by_recipient(coach).await.unwrap(),
        [other, first],
        "同じ動画の依頼は 1 件。別の動画の依頼は残す"
    );

    notifications.mark_all_read(coach, at(3)).await.unwrap();
    let read_at: Vec<_> = notifications
        .find_by_recipient(coach)
        .await
        .unwrap()
        .iter()
        .map(Notification::read_at)
        .collect();
    assert_eq!(
        read_at,
        [Some(at(3)), Some(at(3))],
        "コーチも自分の分を既読にできる"
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn marking_all_read_keeps_the_first_time_and_skips_later_ones(pool: PgPool) {
    let notifications = PgNotificationRepository::new(pool.clone());
    let (student, coach) = (support::student(&pool).await, support::coach(&pool).await);
    let [confirmed, received, returned, from_coach, ..] = pushes(student, coach);
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

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn an_announcement_round_trips_once_per_student(pool: PgPool) {
    let notifications = PgNotificationRepository::new(pool.clone());
    let announcements = PgAnnouncementRepository::new(pool.clone());
    let headquarters = support::register(&pool, Role::Headquarters, vec![]).await;
    let (student, other, coach) = (
        support::student(&pool).await,
        support::student(&pool).await,
        support::coach(&pool).await,
    );
    let sent = Announcement::new(
        announcements.next_id(),
        headquarters,
        AnnouncementTitle::new("メンテナンスのお知らせ").unwrap(),
        AnnouncementBody::new("11/3 は使えません").unwrap(),
        None,
        2,
        at(3),
    )
    .unwrap();
    announcements.insert(sent.clone()).await.unwrap();

    let [confirmed, ..] = pushes(student, coach);
    let first = Notification::new(notifications.next_id(), confirmed, at(1));
    let to_student = Notification::new(notifications.next_id(), sent.push_to(student), at(3));
    let to_other = Notification::new(notifications.next_id(), sent.push_to(other), at(3));
    for notification in [first, to_student, to_other] {
        notifications.insert(notification).await.unwrap();
    }
    // 同じお知らせは同じ生徒に 1 件（障害にもしない）
    notifications
        .insert(Notification::new(
            notifications.next_id(),
            sent.push_to(student),
            at(4),
        ))
        .await
        .unwrap();

    assert_eq!(
        notifications.find_by_recipient(student).await.unwrap(),
        [to_student, first],
        "本部からのお知らせも新しい順に並ぶ"
    );
    assert_eq!(
        notifications.find_by_recipient(other).await.unwrap(),
        [to_other]
    );

    notifications.mark_all_read(student, at(5)).await.unwrap();
    let read = notifications.find_by_recipient(student).await.unwrap();
    assert_eq!(read[0].push(), sent.push_to(student));
    assert_eq!(read[0].read_at(), Some(at(5)));
}
