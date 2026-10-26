//! インメモリのイベントバス（`InMemoryDomainEventBus`）が、発行されたイベントを購読側にその場で渡すこと。
//! entitlement の購読（`LiveLessonQuotaSubscriber`）につなぎ、予約の確定・キャンセルでビデオ通話の暦月の枠が
//! 動くことと、購読側の失敗が発行元に伝わらないことを確かめる。

use std::{
    error::Error,
    sync::Mutex,
    time::{Duration, SystemTime},
};

use application::{
    domain_event::{DomainEvent, DomainEventPublisher, DomainEventSubscriber},
    entitlement::LiveLessonQuotaSubscriber,
    in_memory::InMemoryEntitlementRepository,
};
use domain::{
    entitlement::{Entitlement, EntitlementId, LiveLessonQuota, SubscriptionId, YearMonth},
    identity::{AccountId, Sport},
    scheduling::BookingId,
};
use infra::InMemoryDomainEventBus;

const STUDENT: AccountId = AccountId::from_u128(1);
const COACH: AccountId = AccountId::from_u128(9);

/// 2026-10-26T00:00:00Z
fn oct26() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
}

/// infra は `uuid` クレートに直接依存しないので、sqlx が再公開する `Uuid` で作る（同じ型）。
fn event_id(value: u128) -> sqlx::types::Uuid {
    sqlx::types::Uuid::from_u128(value)
}

fn confirmed(booking: u128) -> DomainEvent {
    DomainEvent::BookingConfirmed {
        event_id: event_id(booking),
        occurred_at: oct26(),
        student_id: STUDENT,
        coach_id: COACH,
        booking_id: BookingId::from_u128(booking),
    }
}

fn cancelled(booking: u128, consumes_quota: bool) -> DomainEvent {
    DomainEvent::BookingCancelled {
        event_id: event_id(100 + booking),
        occurred_at: oct26(),
        student_id: STUDENT,
        coach_id: COACH,
        booking_id: BookingId::from_u128(booking),
        consumes_quota,
        cancelled_at: oct26(),
    }
}

/// 受け取ったイベントを記録し、`fail` なら失敗を返す購読側。
#[derive(Default)]
struct Recording {
    events: Mutex<Vec<DomainEvent>>,
    fail: bool,
}

impl DomainEventSubscriber for Recording {
    async fn handle(&self, event: &DomainEvent) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.events.lock().unwrap().push(event.clone());
        if self.fail {
            Err("購読側の障害".into())
        } else {
            Ok(())
        }
    }
}

#[tokio::test]
async fn delivers_each_published_event_to_the_subscriber_in_order() {
    let bus = InMemoryDomainEventBus::new(Recording::default());
    let copy = bus.clone();
    bus.publish(confirmed(1)).await;
    copy.publish(cancelled(1, true)).await;
    assert_eq!(
        *bus.subscriber().events.lock().unwrap(),
        [confirmed(1), cancelled(1, true)],
        "複製も同じ購読側に渡す"
    );
}

#[tokio::test]
async fn subscriber_failure_does_not_reach_the_publisher() {
    let bus = InMemoryDomainEventBus::new(Recording {
        fail: true,
        ..Recording::default()
    });
    // 失敗しても publish は戻る（戻り値はない）
    bus.publish(confirmed(1)).await;
    assert_eq!(bus.subscriber().events.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn entitlement_consumes_the_live_lesson_quota_through_the_bus() {
    let entitlements = InMemoryEntitlementRepository::with_entitlements([Entitlement::monthly(
        EntitlementId::from_u128(10),
        STUDENT,
        Sport::Tennis,
        SubscriptionId::from_u128(20),
        LiveLessonQuota::fresh(YearMonth::new(2026, 10).unwrap()),
    )
    .unwrap()]);
    let bus = InMemoryDomainEventBus::new(LiveLessonQuotaSubscriber::new(entitlements.clone()));
    let used = || {
        entitlements.all()[0]
            .live_lesson_quota()
            .map(LiveLessonQuota::used)
    };

    bus.publish(confirmed(1)).await;
    assert_eq!(used(), Some(1), "確定で枠を使う");
    bus.publish(cancelled(2, true)).await;
    assert_eq!(
        used(),
        Some(2),
        "確定前の予約の 23 時間前のキャンセルで枠が減る"
    );
    bus.publish(confirmed(3)).await;
    assert_eq!(
        used(),
        Some(2),
        "3 回目は使えない（購読側の失敗として捨てる）"
    );
    bus.publish(cancelled(1, false)).await;
    assert_eq!(used(), Some(1), "24 時間より前のキャンセルで確定の分が戻る");
}
