//! `PgMessageThreadRepository`（`messaging.threads`・`messages`）の検証。スレッドを開く（同じ生徒とコーチは 1 つ）・
//! 送信と既読の往復・当事者の一覧・拒否した送信が何も保存しないこと・並行な送信が両方残ること・添削依頼と添削の返却の
//! メッセージの往復（同じ動画は 1 通ずつ）・境界間イベントの購読でスレッドに足されること・システムのメッセージを読めること。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::{
    async_lesson::{video_id_uuid, video_review_id_uuid},
    domain_event::{DomainEvent, DomainEventSubscriber},
    messaging::{
        MessageThreadRepository, MessageThreadSubscriber, UpdateMessageThreadError,
        message_thread_id_uuid,
    },
};
use domain::{
    async_lesson::{VideoId, VideoReviewId},
    messaging::{
        Message, MessageAuthor, MessageBody, MessageContent, MessageKind, MessageSender,
        MessageThread, MessagingError, Stamp,
    },
};
use infra::PgMessageThreadRepository;
use sqlx::PgPool;
use support::at;

const STUDENT: MessageAuthor = MessageAuthor::Participant(MessageSender::Student);
const COACH: MessageAuthor = MessageAuthor::Participant(MessageSender::Coach);

fn body(text: &str) -> MessageBody {
    MessageBody::new(text).unwrap()
}

fn video_id() -> VideoId {
    video_id_uuid::from_uuid(uuid::Uuid::new_v4())
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_thread_is_opened_once_per_student_and_coach(pool: PgPool) {
    let threads = PgMessageThreadRepository::new(pool.clone());
    let (student, coach) = (support::student(&pool).await, support::coach(&pool).await);
    let thread = MessageThread::open(threads.next_thread_id(), student, coach, at(1)).unwrap();
    assert_eq!(threads.open(thread.clone()).await.unwrap(), thread);

    let again = MessageThread::open(threads.next_thread_id(), student, coach, at(2)).unwrap();
    assert_eq!(
        threads.open(again).await.unwrap(),
        thread,
        "同じ生徒とコーチは既存のスレッドを返す"
    );
    assert_eq!(threads.find(thread.id()).await.unwrap(), Some(thread));
    assert_eq!(threads.find(threads.next_thread_id()).await.unwrap(), None);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn messages_and_reads_round_trip(pool: PgPool) {
    let threads = PgMessageThreadRepository::new(pool.clone());
    let (student, coach) = (support::student(&pool).await, support::coach(&pool).await);
    let thread = threads
        .open(MessageThread::open(threads.next_thread_id(), student, coach, at(1)).unwrap())
        .await
        .unwrap();

    // 同じ送信時刻のメッセージも送った順に読める
    for (sender, text) in [(student, "1"), (coach, "2"), (student, "3")] {
        let id = threads.next_message_id();
        threads
            .update(thread.id(), |t| {
                t.send(id, sender, MessageContent::Text(body(text)), true, at(2))
                    .map(|_| ())
            })
            .await
            .unwrap();
    }
    let read = threads
        .update(thread.id(), |t| t.mark_read(coach, at(3)).map(|_| ()))
        .await
        .unwrap();
    let stored = threads.find(thread.id()).await.unwrap().unwrap();
    assert_eq!(stored, read);
    assert_eq!(
        stored
            .messages()
            .iter()
            .map(|m| (m.author(), m.content().clone(), m.read_at()))
            .collect::<Vec<_>>(),
        [
            (STUDENT, MessageContent::Text(body("1")), Some(at(3))),
            (COACH, MessageContent::Text(body("2")), None),
            (STUDENT, MessageContent::Text(body("3")), Some(at(3))),
        ]
    );
    assert_eq!(stored.unread_count_for(coach), 0);
    assert_eq!(stored.unread_count_for(student), 1);

    // 拒否した送信は何も保存しない
    let before = stored.clone();
    let result = threads
        .update(thread.id(), |t| {
            t.send(
                threads.next_message_id(),
                student,
                MessageContent::Text(body("x")),
                false,
                at(4),
            )
            .map(|_| ())
        })
        .await;
    assert!(
        matches!(
            result,
            Err(UpdateMessageThreadError::Rejected(
                MessagingError::ChatNotAllowed
            ))
        ),
        "{result:?}"
    );
    assert_eq!(threads.find(thread.id()).await.unwrap(), Some(before));
    assert!(matches!(
        threads.update(threads.next_thread_id(), |_| Ok(())).await,
        Err(UpdateMessageThreadError::NotFound)
    ));
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn list_for_returns_the_threads_of_the_participant(pool: PgPool) {
    let threads = PgMessageThreadRepository::new(pool.clone());
    let student = support::student(&pool).await;
    let other_student = support::student(&pool).await;
    let (coach, other_coach) = (support::coach(&pool).await, support::coach(&pool).await);
    let mut opened = Vec::new();
    for (s, c) in [
        (student, coach),
        (student, other_coach),
        (other_student, coach),
    ] {
        opened.push(
            threads
                .open(MessageThread::open(threads.next_thread_id(), s, c, at(1)).unwrap())
                .await
                .unwrap(),
        );
    }
    let id = threads.next_message_id();
    let talked = threads
        .update(opened[0].id(), |t| {
            t.send(
                id,
                coach,
                MessageContent::Text(body("こんにちは")),
                true,
                at(2),
            )
            .map(|_| ())
        })
        .await
        .unwrap();

    let mut of_student = threads.list_for(student).await.unwrap();
    of_student.sort_by_key(MessageThread::id);
    let mut expected = vec![talked.clone(), opened[1].clone()];
    expected.sort_by_key(MessageThread::id);
    assert_eq!(of_student, expected);
    let of_coach = threads.list_for(coach).await.unwrap();
    assert_eq!(of_coach.len(), 2);
    assert!(of_coach.contains(&talked));
    assert!(
        threads
            .list_for(support::student(&pool).await)
            .await
            .unwrap()
            .is_empty()
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn concurrent_sends_are_both_kept(pool: PgPool) {
    let threads = PgMessageThreadRepository::new(pool.clone());
    let (student, coach) = (support::student(&pool).await, support::coach(&pool).await);
    let thread = threads
        .open(MessageThread::open(threads.next_thread_id(), student, coach, at(1)).unwrap())
        .await
        .unwrap();
    let thread_id = thread.id();
    let send = |sender, text: &'static str| {
        let threads = threads.clone();
        let id = threads.next_message_id();
        async move {
            threads
                .update(thread_id, |t| {
                    t.send(id, sender, MessageContent::Text(body(text)), true, at(2))
                        .map(|_| ())
                })
                .await
        }
    };
    let (a, b) = tokio::join!(send(student, "a"), send(coach, "b"));
    a.unwrap();
    b.unwrap();
    let stored = threads.find(thread.id()).await.unwrap().unwrap();
    let mut texts: Vec<&str> = stored
        .messages()
        .iter()
        .map(|m| m.content().body().unwrap().as_str())
        .collect();
    texts.sort_unstable();
    assert_eq!(texts, ["a", "b"]);
    assert!(
        stored
            .messages()
            .iter()
            .all(|m| Message::read_at(m).is_none())
    );
}

/// スレッド `thread` のメッセージの行の数（種類 `kind`）。
async fn count_of(pool: &PgPool, thread: &MessageThread, kind: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM messaging.messages WHERE thread_id = $1 AND kind = $2")
        .bind(message_thread_id_uuid::to_uuid(thread.id()))
        .bind(kind)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn video_review_messages_round_trip_once_per_video(pool: PgPool) {
    let threads = PgMessageThreadRepository::new(pool.clone());
    let (student, coach) = (support::student(&pool).await, support::coach(&pool).await);
    let thread = threads
        .open(MessageThread::open(threads.next_thread_id(), student, coach, at(1)).unwrap())
        .await
        .unwrap();
    let (video, review) = (
        video_id(),
        video_review_id_uuid::from_uuid(uuid::Uuid::new_v4()),
    );

    let request = threads.next_message_id();
    threads
        .update(thread.id(), |t| {
            t.post_video_review_request(request, video, at(2))
                .map(|_| ())
        })
        .await
        .unwrap();
    let text = threads.next_message_id();
    threads
        .update(thread.id(), |t| {
            t.send(
                text,
                coach,
                MessageContent::Text(body("拝見します")),
                true,
                at(3),
            )
            .map(|_| ())
        })
        .await
        .unwrap();
    let returned = threads.next_message_id();
    let saved = threads
        .update(thread.id(), |t| {
            t.post_video_review_return(returned, video, review, at(4))
                .map(|_| ())
        })
        .await
        .unwrap();

    let stored = threads.find(thread.id()).await.unwrap().unwrap();
    assert_eq!(stored, saved);
    assert_eq!(
        stored
            .messages()
            .iter()
            .map(|m| (m.id(), m.author(), m.content().clone(), m.sent_at()))
            .collect::<Vec<_>>(),
        [
            (
                request,
                STUDENT,
                MessageContent::VideoReviewRequest { video_id: video },
                at(2)
            ),
            (text, COACH, MessageContent::Text(body("拝見します")), at(3)),
            (
                returned,
                COACH,
                MessageContent::VideoReviewReturn {
                    video_id: video,
                    review_id: review
                },
                at(4)
            ),
        ]
    );
    assert_eq!(stored.unread_count_for(coach), 1, "生徒の依頼");
    assert_eq!(stored.unread_count_for(student), 2, "コーチの本文と返却");

    // 同じ動画の依頼・返却は 2 通目を足さない（行も増えない）
    for _ in 0..2 {
        let id = threads.next_message_id();
        threads
            .update(thread.id(), |t| {
                t.post_video_review_request(id, video, at(5))?;
                t.post_video_review_return(id, video, review, at(5))
                    .map(|_| ())
            })
            .await
            .unwrap();
    }
    assert_eq!(threads.find(thread.id()).await.unwrap(), Some(stored));
    assert_eq!(count_of(&pool, &thread, "video_review_request").await, 1);
    assert_eq!(count_of(&pool, &thread, "video_review_return").await, 1);
    assert_eq!(count_of(&pool, &thread, "text").await, 1);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn the_subscriber_posts_requests_and_returns_into_the_thread_once(pool: PgPool) {
    let (student, coach) = (support::student(&pool).await, support::coach(&pool).await);
    let subscriber = MessageThreadSubscriber::new(PgMessageThreadRepository::new(pool.clone()));
    let (video, review) = (video_id(), VideoReviewId::from_u128(7));
    let requested = DomainEvent::VideoReviewRequested {
        event_id: uuid::Uuid::new_v4(),
        occurred_at: at(2),
        student_id: student,
        coach_id: coach,
        video_id: video,
    };
    let returned = DomainEvent::VideoReviewReturned {
        event_id: uuid::Uuid::new_v4(),
        occurred_at: at(30),
        student_id: student,
        coach_id: coach,
        video_id: video,
        review_id: review,
    };
    // スレッドがなければ依頼の時刻に開き、同じイベントが 2 回届いても 1 通
    for event in [&requested, &requested, &returned, &returned] {
        subscriber.handle(event).await.unwrap();
    }

    let threads = subscriber.threads().list_for(student).await.unwrap();
    assert_eq!(threads.len(), 1);
    let thread = &threads[0];
    assert_eq!(
        (thread.student_id(), thread.coach_id(), thread.opened_at()),
        (student, coach, at(2))
    );
    assert_eq!(
        thread
            .messages()
            .iter()
            .map(|m| (m.author(), m.content().kind(), m.sent_at()))
            .collect::<Vec<_>>(),
        [
            (STUDENT, MessageKind::VideoReviewRequest, at(2)),
            (COACH, MessageKind::VideoReviewReturn, at(30)),
        ]
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn system_messages_and_old_rows_are_read(pool: PgPool) {
    let threads = PgMessageThreadRepository::new(pool.clone());
    let (student, coach) = (support::student(&pool).await, support::coach(&pool).await);
    let thread = threads
        .open(MessageThread::open(threads.next_thread_id(), student, coach, at(1)).unwrap())
        .await
        .unwrap();
    // 古い版のアプリの書き方（kind を書かない）は本文の種類になり、システムの行も読める
    for (sender_role, text) in [("student", "古い版から"), ("system", "お知らせ")] {
        sqlx::query(
            "INSERT INTO messaging.messages (thread_id, sender_role, body, sent_at)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(message_thread_id_uuid::to_uuid(thread.id()))
        .bind(sender_role)
        .bind(text)
        .bind(support_timestamp(at(2)))
        .execute(&pool)
        .await
        .unwrap();
    }
    let stored = threads.find(thread.id()).await.unwrap().unwrap();
    assert_eq!(
        stored
            .messages()
            .iter()
            .map(|m| (m.author(), m.content().clone()))
            .collect::<Vec<_>>(),
        [
            (STUDENT, MessageContent::Text(body("古い版から"))),
            (
                MessageAuthor::System,
                MessageContent::Text(body("お知らせ"))
            ),
        ]
    );
    assert_eq!(
        stored.unread_count_for(coach),
        1,
        "システムのものは数えない"
    );
    assert_eq!(stored.unread_count_for(student), 0);
    // 既読にしてもシステムのものは変えない
    let read = threads
        .update(thread.id(), |t| t.mark_read(coach, at(3)).map(|_| ()))
        .await
        .unwrap();
    assert_eq!(
        read.messages()
            .iter()
            .map(Message::read_at)
            .collect::<Vec<_>>(),
        [Some(at(3)), None]
    );
    assert_eq!(threads.find(thread.id()).await.unwrap(), Some(read));
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn stamps_round_trip_with_every_id(pool: PgPool) {
    let threads = PgMessageThreadRepository::new(pool.clone());
    let (student, coach) = (support::student(&pool).await, support::coach(&pool).await);
    let thread = threads
        .open(MessageThread::open(threads.next_thread_id(), student, coach, at(1)).unwrap())
        .await
        .unwrap();
    for (i, stamp) in Stamp::ALL.into_iter().enumerate() {
        let sender = if i % 2 == 0 { student } else { coach };
        let id = threads.next_message_id();
        threads
            .update(thread.id(), |t| {
                t.send(id, sender, MessageContent::Stamp(stamp), true, at(2))
                    .map(|_| ())
            })
            .await
            .unwrap();
    }
    let stored = threads.find(thread.id()).await.unwrap().unwrap();
    assert_eq!(
        stored
            .messages()
            .iter()
            .map(|m| (m.author(), m.content().clone()))
            .collect::<Vec<_>>(),
        Stamp::ALL
            .into_iter()
            .enumerate()
            .map(|(i, stamp)| (
                if i % 2 == 0 { STUDENT } else { COACH },
                MessageContent::Stamp(stamp)
            ))
            .collect::<Vec<_>>()
    );
    assert!(
        stored
            .messages()
            .iter()
            .all(|m| m.content().kind() == MessageKind::Stamp && m.content().body().is_none())
    );
    assert_eq!(stored.unread_count_for(coach), 4);
    assert_eq!(stored.unread_count_for(student), 4);
}

fn support_timestamp(at: std::time::SystemTime) -> sqlx::types::time::OffsetDateTime {
    at.into()
}
