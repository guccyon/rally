use std::{error::Error, time::SystemTime};

use domain::{identity::AccountId, scheduling::BookingId};

use super::{
    ConsumeLiveLessonQuota, ConsumeLiveLessonQuotaCommand, ConsumeLiveLessonQuotaError,
    EntitlementRepository, ReleaseLiveLessonQuota, ReleaseLiveLessonQuotaCommand,
    ReleaseLiveLessonQuotaError,
};
use crate::domain_event::{DomainEvent, DomainEventSubscriber};

/// entitlement による境界間イベントの購読。ビデオ通話の暦月の枠を予約の出来事に合わせて動かす。
///
/// - `BookingConfirmed`: 確定した予約で枠を 1 回使う（[`ConsumeLiveLessonQuota`]。暦月は `occurred_at`）
/// - `BookingCancelled` の `consumes_quota: true`（開始の 24 時間前以降）: その予約で枠を使ったことにする
///   （確定で使っていればそのまま、確定前なら 1 回使う。暦月は `cancelled_at`）
/// - `BookingCancelled` の `consumes_quota: false`: その予約で使った枠を戻す（[`ReleaseLiveLessonQuota`]）
///
/// 回数を消費するかは scheduling の判定（`consumes_quota`）に従い、ここでは判定し直さない（ADR 0002 Q2）。
/// 受講権のない生徒（月額契約の前）の予約は何もしない。同じ予約で 2 回は使わないので、同じイベントが
/// 2 回届いても結果は変わらない。
pub struct LiveLessonQuotaSubscriber<E> {
    consume: ConsumeLiveLessonQuota<E>,
    release: ReleaseLiveLessonQuota<E>,
}

impl<E> LiveLessonQuotaSubscriber<E>
where
    E: EntitlementRepository + Clone,
{
    pub fn new(entitlements: E) -> Self {
        Self {
            consume: ConsumeLiveLessonQuota::new(entitlements.clone()),
            release: ReleaseLiveLessonQuota::new(entitlements),
        }
    }
}

impl<E> DomainEventSubscriber for LiveLessonQuotaSubscriber<E>
where
    E: EntitlementRepository,
{
    async fn handle(&self, event: &DomainEvent) -> Result<(), Box<dyn Error + Send + Sync>> {
        match *event {
            DomainEvent::BookingConfirmed {
                occurred_at,
                student_id,
                booking_id,
                ..
            } => self.consume(student_id, booking_id, occurred_at).await,
            DomainEvent::BookingCancelled {
                student_id,
                booking_id,
                consumes_quota: true,
                cancelled_at,
                ..
            } => self.consume(student_id, booking_id, cancelled_at).await,
            DomainEvent::BookingCancelled {
                student_id,
                booking_id,
                consumes_quota: false,
                cancelled_at,
                ..
            } => {
                let result = self
                    .release
                    .execute(ReleaseLiveLessonQuotaCommand {
                        student_id,
                        booking_id,
                        at: cancelled_at,
                    })
                    .await;
                match result {
                    Ok(_) | Err(ReleaseLiveLessonQuotaError::NotFound) => Ok(()),
                    Err(ReleaseLiveLessonQuotaError::Unavailable(error)) => Err(error.into()),
                }
            }
            DomainEvent::LessonCompleted { .. } | DomainEvent::SubscriptionStarted { .. } => Ok(()),
        }
    }
}

impl<E> LiveLessonQuotaSubscriber<E>
where
    E: EntitlementRepository,
{
    async fn consume(
        &self,
        student_id: AccountId,
        booking_id: BookingId,
        at: SystemTime,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let result = self
            .consume
            .execute(ConsumeLiveLessonQuotaCommand {
                student_id,
                booking_id,
                at,
            })
            .await;
        match result {
            Ok(_) | Err(ConsumeLiveLessonQuotaError::NotFound) => Ok(()),
            Err(ConsumeLiveLessonQuotaError::Invalid(error)) => Err(error.into()),
            Err(ConsumeLiveLessonQuotaError::Unavailable(error)) => Err(error.into()),
        }
    }

    pub fn entitlements(&self) -> &E {
        self.consume.entitlements()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use domain::{
        entitlement::{Entitlement, EntitlementId, LiveLessonQuota, SubscriptionId, YearMonth},
        identity::Sport,
    };
    use uuid::Uuid;

    use super::*;
    use crate::in_memory::InMemoryEntitlementRepository;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(9);

    /// 2026-10-26T00:00:00Z
    fn oct26() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
    }

    fn subscriber() -> LiveLessonQuotaSubscriber<InMemoryEntitlementRepository> {
        let monthly = Entitlement::monthly(
            EntitlementId::from_u128(10),
            STUDENT,
            Sport::Tennis,
            SubscriptionId::from_u128(20),
            LiveLessonQuota::fresh(YearMonth::new(2026, 10).unwrap()),
        )
        .unwrap();
        LiveLessonQuotaSubscriber::new(InMemoryEntitlementRepository::with_entitlements([monthly]))
    }

    fn confirmed(student_id: AccountId, booking: u128) -> DomainEvent {
        DomainEvent::BookingConfirmed {
            event_id: Uuid::new_v4(),
            occurred_at: oct26(),
            student_id,
            coach_id: COACH,
            booking_id: BookingId::from_u128(booking),
        }
    }

    fn cancelled(booking: u128, consumes_quota: bool) -> DomainEvent {
        DomainEvent::BookingCancelled {
            event_id: Uuid::new_v4(),
            occurred_at: oct26(),
            student_id: STUDENT,
            coach_id: COACH,
            booking_id: BookingId::from_u128(booking),
            consumes_quota,
            cancelled_at: oct26(),
        }
    }

    fn used(subscriber: &LiveLessonQuotaSubscriber<InMemoryEntitlementRepository>) -> u8 {
        subscriber.entitlements().all()[0]
            .live_lesson_quota()
            .unwrap()
            .used()
    }

    #[tokio::test]
    async fn confirmation_consumes_and_the_third_is_an_error() {
        let subscriber = subscriber();
        subscriber.handle(&confirmed(STUDENT, 1)).await.unwrap();
        subscriber.handle(&confirmed(STUDENT, 1)).await.unwrap();
        assert_eq!(used(&subscriber), 1, "同じ予約の確定が 2 回届いても 1 回分");
        subscriber.handle(&confirmed(STUDENT, 2)).await.unwrap();
        assert_eq!(used(&subscriber), 2);
        assert!(subscriber.handle(&confirmed(STUDENT, 3)).await.is_err());
        assert_eq!(used(&subscriber), 2);
    }

    #[tokio::test]
    async fn late_cancellation_consumes_and_early_cancellation_releases() {
        let subscriber = subscriber();
        // 確定前の予約を 23 時間前にキャンセル: 枠が減る
        subscriber.handle(&cancelled(1, true)).await.unwrap();
        assert_eq!(used(&subscriber), 1);

        // 確定した予約を 23 時間前にキャンセル: 確定で使った分のまま
        subscriber.handle(&confirmed(STUDENT, 2)).await.unwrap();
        subscriber.handle(&cancelled(2, true)).await.unwrap();
        assert_eq!(used(&subscriber), 2);

        // 25 時間前のキャンセル: 確定で使った分が戻る
        subscriber.handle(&cancelled(2, false)).await.unwrap();
        assert_eq!(used(&subscriber), 1);
    }

    #[tokio::test]
    async fn students_without_an_entitlement_and_other_events_change_nothing() {
        let subscriber = subscriber();
        subscriber
            .handle(&confirmed(AccountId::from_u128(2), 1))
            .await
            .unwrap();
        subscriber
            .handle(&DomainEvent::SubscriptionStarted {
                event_id: Uuid::new_v4(),
                occurred_at: oct26(),
                student_id: STUDENT,
                subscription_id: SubscriptionId::from_u128(20),
            })
            .await
            .unwrap();
        assert_eq!(used(&subscriber), 0);
    }
}
