use std::{error::Error, time::SystemTime};

use domain::{
    identity::AccountId,
    messaging::{MessageId, MessageThread, MessagingError},
};

use super::{
    MessageThreadRepository, MessageThreadSubscriberError, OpenMessageThread,
    OpenMessageThreadCommand, OpenMessageThreadError, UpdateMessageThreadError,
};
use crate::domain_event::{DomainEvent, DomainEventSubscriber};

/// messaging による境界間イベントの購読。
///
/// - 予約の確定（`BookingConfirmed`）で、その生徒とコーチのスレッドを開く（[`OpenMessageThread`]。開いた時刻は
///   `occurred_at`）。Gen1 は担当コーチを確定する matching がないので、確定した予約のコーチを担当コーチとみなす
///   （予約をキャンセルしてもスレッドは閉じない）
/// - 添削依頼（async-lesson の `VideoReviewRequested`）と添削の返却（`VideoReviewReturned`）で、その生徒とコーチの
///   スレッドに（なければ `occurred_at` に開いて）添削依頼・添削の返却のメッセージを足す（送信時刻は `occurred_at`）。
///   チャットは会話でありレッスンのログでもある（docs/adr/0009 の答え合わせ）。メッセージは動画・添削を ID で指すだけで、
///   添削の有無・回数・状態は async-lesson が持つ。チャットの送り放題の判定（`ChatGate`）は通さず、境界間イベント
///   `MessageSent` と行動イベント `message.sent` も出さない（当事者が送ったのではなく、プッシュ通知は
///   notification が async-lesson のイベントから送る）
///
/// 同じ生徒とコーチのスレッドは 1 つで、同じ動画の添削依頼・返却はスレッドに 1 通ずつなので、同じイベントが 2 回
/// 届いても結果は変わらない。ほかのイベントは何もしない。
///
/// 失敗は型のあるエラーを包んで返す（不変条件の違反は `MessagingError`、保存先の障害は [`crate::Unavailable`]、
/// 開いたスレッドがメッセージを足すときになかったら [`MessageThreadSubscriberError::ThreadVanished`]）。
pub struct MessageThreadSubscriber<T> {
    open: OpenMessageThread<T>,
}

impl<T> MessageThreadSubscriber<T>
where
    T: MessageThreadRepository,
{
    pub fn new(threads: T) -> Self {
        Self {
            open: OpenMessageThread::new(threads),
        }
    }

    pub fn threads(&self) -> &T {
        self.open.threads()
    }

    /// 生徒 `student_id` とコーチ `coach_id` のスレッドを（なければ `at` に開いて）返す。
    async fn open(
        &self,
        student_id: AccountId,
        coach_id: AccountId,
        at: SystemTime,
    ) -> Result<MessageThread, Box<dyn Error + Send + Sync>> {
        self.open
            .execute(OpenMessageThreadCommand {
                student_id,
                coach_id,
                at,
            })
            .await
            .map_err(|error| match error {
                OpenMessageThreadError::Invalid(error) => error.into(),
                OpenMessageThreadError::Unavailable(error) => error.into(),
            })
    }

    /// 生徒 `student_id` とコーチ `coach_id` のスレッドを（なければ `at` に開いて）、`post` でメッセージを足して保存する。
    /// `post` は新しいメッセージの ID を受け取る。
    async fn post<F>(
        &self,
        student_id: AccountId,
        coach_id: AccountId,
        at: SystemTime,
        post: F,
    ) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        F: FnOnce(&mut MessageThread, MessageId) -> Result<(), MessagingError> + Send,
    {
        let thread = self.open(student_id, coach_id, at).await?;
        let message_id = self.threads().next_message_id();
        self.threads()
            .update(thread.id(), |thread| post(thread, message_id))
            .await
            .map(|_| ())
            .map_err(|error| match error {
                UpdateMessageThreadError::NotFound => {
                    MessageThreadSubscriberError::ThreadVanished.into()
                }
                UpdateMessageThreadError::Rejected(error) => error.into(),
                UpdateMessageThreadError::Unavailable(error) => error.into(),
            })
    }
}

impl<T> DomainEventSubscriber for MessageThreadSubscriber<T>
where
    T: MessageThreadRepository,
{
    async fn handle(&self, event: &DomainEvent) -> Result<(), Box<dyn Error + Send + Sync>> {
        match *event {
            DomainEvent::BookingConfirmed {
                occurred_at,
                student_id,
                coach_id,
                ..
            } => self
                .open(student_id, coach_id, occurred_at)
                .await
                .map(|_| ()),
            DomainEvent::VideoReviewRequested {
                occurred_at,
                student_id,
                coach_id,
                video_id,
                ..
            } => {
                self.post(student_id, coach_id, occurred_at, |thread, message_id| {
                    thread
                        .post_video_review_request(message_id, video_id, occurred_at)
                        .map(|_| ())
                })
                .await
            }
            DomainEvent::VideoReviewReturned {
                occurred_at,
                student_id,
                coach_id,
                video_id,
                review_id,
                ..
            } => {
                self.post(student_id, coach_id, occurred_at, |thread, message_id| {
                    thread
                        .post_video_review_return(message_id, video_id, review_id, occurred_at)
                        .map(|_| ())
                })
                .await
            }
            DomainEvent::BookingCancelled { .. }
            | DomainEvent::LessonCompleted { .. }
            | DomainEvent::SubscriptionStarted { .. }
            | DomainEvent::MessageSent { .. } => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::{VideoId, VideoReviewId},
        messaging::{MessageAuthor, MessageContent, MessageSender},
        scheduling::BookingId,
    };
    use uuid::Uuid;

    use super::*;
    use crate::in_memory::InMemoryMessageThreadRepository;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(9);

    fn oct26() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
    }

    fn confirmed(booking: u128) -> DomainEvent {
        DomainEvent::BookingConfirmed {
            event_id: Uuid::new_v4(),
            occurred_at: oct26(),
            student_id: STUDENT,
            coach_id: COACH,
            booking_id: BookingId::from_u128(booking),
        }
    }

    #[tokio::test]
    async fn a_confirmed_booking_opens_a_thread_once() {
        let subscriber = MessageThreadSubscriber::new(InMemoryMessageThreadRepository::default());
        subscriber.handle(&confirmed(1)).await.unwrap();
        subscriber.handle(&confirmed(1)).await.unwrap();
        subscriber.handle(&confirmed(2)).await.unwrap();

        let threads = subscriber.threads().all();
        assert_eq!(threads.len(), 1, "同じ生徒とコーチのスレッドは 1 つ");
        assert_eq!(
            (
                threads[0].student_id(),
                threads[0].coach_id(),
                threads[0].opened_at()
            ),
            (STUDENT, COACH, oct26())
        );
    }

    #[tokio::test]
    async fn other_events_open_nothing() {
        let subscriber = MessageThreadSubscriber::new(InMemoryMessageThreadRepository::default());
        subscriber
            .handle(&DomainEvent::BookingCancelled {
                event_id: Uuid::new_v4(),
                occurred_at: oct26(),
                student_id: STUDENT,
                coach_id: COACH,
                booking_id: BookingId::from_u128(1),
                consumes_quota: false,
                cancelled_at: oct26(),
            })
            .await
            .unwrap();
        assert!(subscriber.threads().all().is_empty());
    }

    const VIDEO: VideoId = VideoId::from_u128(40);
    const REVIEW: VideoReviewId = VideoReviewId::from_u128(50);

    fn at(hour: u64) -> SystemTime {
        oct26() + Duration::from_secs(hour * 3600)
    }

    fn requested(event_id: Uuid, hour: u64) -> DomainEvent {
        DomainEvent::VideoReviewRequested {
            event_id,
            occurred_at: at(hour),
            student_id: STUDENT,
            coach_id: COACH,
            video_id: VIDEO,
        }
    }

    fn returned(event_id: Uuid, hour: u64) -> DomainEvent {
        DomainEvent::VideoReviewReturned {
            event_id,
            occurred_at: at(hour),
            student_id: STUDENT,
            coach_id: COACH,
            video_id: VIDEO,
            review_id: REVIEW,
        }
    }

    #[tokio::test]
    async fn a_video_review_request_opens_the_thread_if_there_is_none_and_is_posted_once() {
        let subscriber = MessageThreadSubscriber::new(InMemoryMessageThreadRepository::default());
        let event = requested(Uuid::new_v4(), 1);
        subscriber.handle(&event).await.unwrap();
        subscriber.handle(&event).await.unwrap();

        let threads = subscriber.threads().all();
        assert_eq!(threads.len(), 1);
        let thread = &threads[0];
        assert_eq!(
            (thread.student_id(), thread.coach_id(), thread.opened_at()),
            (STUDENT, COACH, at(1)),
            "スレッドがなければ依頼の時刻に開く"
        );
        assert_eq!(
            thread.messages().len(),
            1,
            "同じイベントが 2 回届いても 1 通"
        );
        let message = &thread.messages()[0];
        assert_eq!(
            message.author(),
            MessageAuthor::Participant(MessageSender::Student)
        );
        assert_eq!(
            message.content(),
            &MessageContent::VideoReviewRequest { video_id: VIDEO }
        );
        assert_eq!(message.sent_at(), at(1));
        assert_eq!(thread.unread_count_for(COACH), 1);
    }

    #[tokio::test]
    async fn the_request_and_the_return_join_the_conversation_of_the_existing_thread() {
        let subscriber = MessageThreadSubscriber::new(InMemoryMessageThreadRepository::default());
        subscriber.handle(&confirmed(1)).await.unwrap();
        subscriber
            .handle(&requested(Uuid::new_v4(), 1))
            .await
            .unwrap();
        let event = returned(Uuid::new_v4(), 30);
        subscriber.handle(&event).await.unwrap();
        subscriber.handle(&event).await.unwrap();

        let threads = subscriber.threads().all();
        assert_eq!(threads.len(), 1, "予約の確定で開いたスレッドに足す");
        assert_eq!(threads[0].opened_at(), oct26());
        let messages: Vec<_> = threads[0]
            .messages()
            .iter()
            .map(|m| (m.author(), m.content().clone(), m.sent_at()))
            .collect();
        assert_eq!(
            messages,
            [
                (
                    MessageAuthor::Participant(MessageSender::Student),
                    MessageContent::VideoReviewRequest { video_id: VIDEO },
                    at(1)
                ),
                (
                    MessageAuthor::Participant(MessageSender::Coach),
                    MessageContent::VideoReviewReturn {
                        video_id: VIDEO,
                        review_id: REVIEW
                    },
                    at(30)
                ),
            ]
        );
        assert_eq!(threads[0].unread_count_for(STUDENT), 1);
    }

    #[tokio::test]
    async fn a_return_without_a_thread_opens_one() {
        let subscriber = MessageThreadSubscriber::new(InMemoryMessageThreadRepository::default());
        subscriber
            .handle(&returned(Uuid::new_v4(), 2))
            .await
            .unwrap();
        let threads = subscriber.threads().all();
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].opened_at(), at(2));
        assert_eq!(threads[0].messages().len(), 1);
    }

    #[tokio::test]
    async fn a_request_with_oneself_is_an_error_and_saves_nothing() {
        let subscriber = MessageThreadSubscriber::new(InMemoryMessageThreadRepository::default());
        let result = subscriber
            .handle(&DomainEvent::VideoReviewRequested {
                event_id: Uuid::new_v4(),
                occurred_at: at(1),
                student_id: STUDENT,
                coach_id: STUDENT,
                video_id: VIDEO,
            })
            .await;
        assert!(result.is_err());
        assert!(subscriber.threads().all().is_empty());
    }

    /// スレッドは開けるが、メッセージを足すときにはなくなっている保存先。
    struct VanishingThreads(InMemoryMessageThreadRepository);

    impl MessageThreadRepository for VanishingThreads {
        fn next_thread_id(&self) -> domain::messaging::MessageThreadId {
            self.0.next_thread_id()
        }

        fn next_message_id(&self) -> MessageId {
            self.0.next_message_id()
        }

        async fn open(&self, thread: MessageThread) -> Result<MessageThread, crate::Unavailable> {
            self.0.open(thread).await
        }

        async fn find(
            &self,
            thread_id: domain::messaging::MessageThreadId,
        ) -> Result<Option<MessageThread>, crate::Unavailable> {
            self.0.find(thread_id).await
        }

        async fn list_for(
            &self,
            account_id: AccountId,
        ) -> Result<Vec<MessageThread>, crate::Unavailable> {
            self.0.list_for(account_id).await
        }

        async fn update<F>(
            &self,
            _thread_id: domain::messaging::MessageThreadId,
            _update: F,
        ) -> Result<MessageThread, UpdateMessageThreadError>
        where
            F: FnOnce(&mut MessageThread) -> Result<(), MessagingError> + Send,
        {
            Err(UpdateMessageThreadError::NotFound)
        }
    }

    #[tokio::test]
    async fn a_thread_that_vanishes_before_posting_is_a_typed_error() {
        let subscriber = MessageThreadSubscriber::new(VanishingThreads(
            InMemoryMessageThreadRepository::default(),
        ));
        let error = subscriber
            .handle(&requested(Uuid::new_v4(), 1))
            .await
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<MessageThreadSubscriberError>(),
            Some(&MessageThreadSubscriberError::ThreadVanished),
            "文言ではなく型で見分けられる"
        );
    }
}
