use std::{collections::HashSet, error::Error, sync::Mutex};

use domain::notification::PushNotification;
use uuid::Uuid;

use super::PushSender;
use crate::domain_event::{DomainEvent, DomainEventSubscriber};

/// notification による境界間イベントの購読。予約の確定（`BookingConfirmed`）で、生徒にプッシュ通知を送る
/// （[`PushNotification::BookingConfirmed`]）。scheduling は notification を直接呼ばない（context-map の向き）。
///
/// 通知は保存しないので、同じイベントの再送で 2 回送らないように、送った境界間イベントの `event_id` を
/// プロセスのメモリに覚える（送れなかったイベントは覚えず、再送で送り直す）。ほかのイベントは何もしない。
pub struct PushNotificationSubscriber<P> {
    sender: P,
    sent_events: Mutex<HashSet<Uuid>>,
}

impl<P> PushNotificationSubscriber<P>
where
    P: PushSender,
{
    pub fn new(sender: P) -> Self {
        Self {
            sender,
            sent_events: Mutex::default(),
        }
    }

    pub fn sender(&self) -> &P {
        &self.sender
    }

    /// `event_id` のイベントの通知を一度だけ送る。同じ `event_id` を送っている途中・送った後なら何もしない。
    async fn send_once(
        &self,
        event_id: Uuid,
        notification: PushNotification,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if !self.sent_events.lock().unwrap().insert(event_id) {
            return Ok(());
        }
        let result = self.sender.send(notification).await;
        if result.is_err() {
            self.sent_events.lock().unwrap().remove(&event_id);
        }
        result.map_err(Into::into)
    }
}

impl<P> DomainEventSubscriber for PushNotificationSubscriber<P>
where
    P: PushSender,
{
    async fn handle(&self, event: &DomainEvent) -> Result<(), Box<dyn Error + Send + Sync>> {
        match *event {
            DomainEvent::BookingConfirmed {
                event_id,
                student_id,
                coach_id,
                booking_id,
                ..
            } => {
                self.send_once(
                    event_id,
                    PushNotification::BookingConfirmed {
                        student_id,
                        coach_id,
                        booking_id,
                    },
                )
                .await
            }
            DomainEvent::BookingCancelled { .. }
            | DomainEvent::LessonCompleted { .. }
            | DomainEvent::SubscriptionStarted { .. } => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        time::{Duration, SystemTime},
    };

    use domain::{identity::AccountId, scheduling::BookingId};

    use super::*;
    use crate::{Unavailable, in_memory::RecordingPushSender};

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(9);

    fn oct26() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
    }

    fn confirmed(event_id: Uuid, booking: u128) -> DomainEvent {
        DomainEvent::BookingConfirmed {
            event_id,
            occurred_at: oct26(),
            student_id: STUDENT,
            coach_id: COACH,
            booking_id: BookingId::from_u128(booking),
        }
    }

    fn notification(booking: u128) -> PushNotification {
        PushNotification::BookingConfirmed {
            student_id: STUDENT,
            coach_id: COACH,
            booking_id: BookingId::from_u128(booking),
        }
    }

    #[tokio::test]
    async fn a_confirmed_booking_notifies_the_student_once_per_event() {
        let subscriber = PushNotificationSubscriber::new(RecordingPushSender::default());
        let first = Uuid::new_v4();
        subscriber.handle(&confirmed(first, 1)).await.unwrap();
        subscriber.handle(&confirmed(first, 1)).await.unwrap();
        subscriber
            .handle(&confirmed(Uuid::new_v4(), 2))
            .await
            .unwrap();

        let sent = subscriber.sender().sent();
        assert_eq!(
            sent,
            [notification(1), notification(2)],
            "同じイベントの再送では送らない"
        );
        assert_eq!(sent[0].recipient(), STUDENT);
    }

    #[tokio::test]
    async fn other_events_send_nothing() {
        let subscriber = PushNotificationSubscriber::new(RecordingPushSender::default());
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
        assert!(subscriber.sender().sent().is_empty());
    }

    /// `down` の間は障害を返し、それ以外は送った通知を記録する送り先。
    #[derive(Default)]
    struct Flaky {
        down: AtomicBool,
        sent: RecordingPushSender,
    }

    impl PushSender for Flaky {
        async fn send(&self, notification: PushNotification) -> Result<(), Unavailable> {
            if self.down.load(Ordering::SeqCst) {
                return Err(Unavailable::new("push gateway is down"));
            }
            self.sent.send(notification).await
        }
    }

    #[tokio::test]
    async fn a_failed_send_is_retried_when_the_event_comes_again() {
        let subscriber = PushNotificationSubscriber::new(Flaky::default());
        subscriber.sender().down.store(true, Ordering::SeqCst);
        let event = confirmed(Uuid::new_v4(), 1);
        assert!(subscriber.handle(&event).await.is_err());
        assert!(subscriber.sender().sent.sent().is_empty());

        subscriber.sender().down.store(false, Ordering::SeqCst);
        subscriber.handle(&event).await.unwrap();
        assert_eq!(subscriber.sender().sent.sent(), [notification(1)]);
    }
}
