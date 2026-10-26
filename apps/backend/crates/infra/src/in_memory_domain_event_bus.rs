//! 境界間イベントのインメモリのイベントバス（メッセージ基盤のモック）。

use std::sync::Arc;

use application::domain_event::{DomainEvent, DomainEventPublisher, DomainEventSubscriber};

use crate::LoggingDomainEventPublisher;

/// [`DomainEventPublisher`] のモック。発行されたイベントを [`LoggingDomainEventPublisher`] と同じ 1 行で標準出力に
/// 書き、同じプロセスの購読側（[`DomainEventSubscriber`]）にその場で渡す。購読側が終わるまで発行は戻らない。
///
/// Gen1 はメッセージ基盤を選定していないので、確実な配送（再送・アウトボックス）はしない。購読側の失敗は
/// 標準エラー出力に書いて捨てる（発行の失敗でユースケースを失敗させない）。複製は同じ購読側を共有する。
#[derive(Debug)]
pub struct InMemoryDomainEventBus<S> {
    subscriber: Arc<S>,
}

impl<S> InMemoryDomainEventBus<S> {
    pub fn new(subscriber: S) -> Self {
        Self {
            subscriber: Arc::new(subscriber),
        }
    }

    pub fn subscriber(&self) -> &S {
        &self.subscriber
    }
}

impl<S> Clone for InMemoryDomainEventBus<S> {
    fn clone(&self) -> Self {
        Self {
            subscriber: self.subscriber.clone(),
        }
    }
}

impl<S> DomainEventPublisher for InMemoryDomainEventBus<S>
where
    S: DomainEventSubscriber,
{
    async fn publish(&self, event: DomainEvent) {
        LoggingDomainEventPublisher.publish(event.clone()).await;
        if let Err(error) = self.subscriber.handle(&event).await {
            eprintln!("domain_event {} の購読に失敗: {error}", event.name());
        }
    }
}
