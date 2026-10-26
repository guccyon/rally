//! ドメインイベントの発行のモック（標準出力へのログ）。

use application::domain_event::{DomainEvent, DomainEventPublisher};

use crate::domain_event_payload::payload;

/// [`DomainEventPublisher`] のモック。Gen1 はメッセージ基盤を選定していないので、イベントを
/// `domain_event <メッセージ名> <ペイロードの JSON>` の 1 行で標準出力に書くだけで、誰にも届けない。
/// ペイロードを作れなければ標準エラー出力に書いて捨てる（発行の失敗でユースケースを失敗させない）。
#[derive(Debug, Clone, Copy, Default)]
pub struct LoggingDomainEventPublisher;

impl LoggingDomainEventPublisher {
    /// 標準出力に書く 1 行。
    pub fn line(event: &DomainEvent) -> Result<String, time::error::Format> {
        Ok(format!("domain_event {} {}", event.name(), payload(event)?))
    }
}

impl DomainEventPublisher for LoggingDomainEventPublisher {
    async fn publish(&self, event: DomainEvent) {
        match Self::line(&event) {
            Ok(line) => println!("{line}"),
            Err(error) => eprintln!("domain_event {} を書けない: {error}", event.name()),
        }
    }
}
