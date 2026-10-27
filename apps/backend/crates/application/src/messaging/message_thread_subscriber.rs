use std::error::Error;

use super::{
    MessageThreadRepository, OpenMessageThread, OpenMessageThreadCommand, OpenMessageThreadError,
};
use crate::domain_event::{DomainEvent, DomainEventSubscriber};

/// messaging による境界間イベントの購読。予約の確定（`BookingConfirmed`）で、その生徒とコーチのスレッドを開く
/// （[`OpenMessageThread`]。開いた時刻は `occurred_at`）。
///
/// Gen1 は担当コーチを確定する matching がないので、確定した予約のコーチを担当コーチとみなす（予約をキャンセル
/// してもスレッドは閉じない）。同じ生徒とコーチのスレッドは 1 つなので、同じイベントが 2 回届いても結果は
/// 変わらない。ほかのイベントは何もしない。
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
            } => {
                let result = self
                    .open
                    .execute(OpenMessageThreadCommand {
                        student_id,
                        coach_id,
                        at: occurred_at,
                    })
                    .await;
                match result {
                    Ok(_) => Ok(()),
                    Err(OpenMessageThreadError::Invalid(error)) => Err(error.into()),
                    Err(OpenMessageThreadError::Unavailable(error)) => Err(error.into()),
                }
            }
            DomainEvent::BookingCancelled { .. }
            | DomainEvent::LessonCompleted { .. }
            | DomainEvent::SubscriptionStarted { .. }
            | DomainEvent::VideoReviewReturned { .. }
            | DomainEvent::MessageSent { .. } => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{identity::AccountId, scheduling::BookingId};
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
}
