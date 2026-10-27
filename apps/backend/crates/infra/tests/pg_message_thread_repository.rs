//! `PgMessageThreadRepository`（`messaging.threads`・`messages`）の検証。スレッドを開く（同じ生徒とコーチは 1 つ）・
//! 送信と既読の往復・当事者の一覧・拒否した送信が何も保存しないこと・並行な送信が両方残ること。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::messaging::{MessageThreadRepository, UpdateMessageThreadError};
use domain::{
    DomainError,
    messaging::{Message, MessageBody, MessageSender, MessageThread},
};
use infra::PgMessageThreadRepository;
use sqlx::PgPool;
use support::at;

fn body(text: &str) -> MessageBody {
    MessageBody::new(text).unwrap()
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
                t.send(id, sender, body(text), true, at(2)).map(|_| ())
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
            .map(|m| (m.sender(), m.body().as_str(), m.read_at()))
            .collect::<Vec<_>>(),
        [
            (MessageSender::Student, "1", Some(at(3))),
            (MessageSender::Coach, "2", None),
            (MessageSender::Student, "3", Some(at(3))),
        ]
    );
    assert_eq!(stored.unread_count_for(coach), 0);
    assert_eq!(stored.unread_count_for(student), 1);

    // 拒否した送信は何も保存しない
    let before = stored.clone();
    let result = threads
        .update(thread.id(), |t| {
            t.send(threads.next_message_id(), student, body("x"), false, at(4))
                .map(|_| ())
        })
        .await;
    assert!(
        matches!(
            result,
            Err(UpdateMessageThreadError::Rejected(
                DomainError::ChatNotAllowed
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
            t.send(id, coach, body("こんにちは"), true, at(2))
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
                    t.send(id, sender, body(text), true, at(2)).map(|_| ())
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
        .map(|m| m.body().as_str())
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
