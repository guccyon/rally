//! 境界づけられたコンテキスト間のドメインイベント（specs/contracts/events/asyncapi.yaml）を発行するポート。
//!
//! 行動イベント（[`crate::analytics`]）とは別物。Gen1 のメッセージ基盤は選定しておらず、実装は infra の
//! モック（ログ出力）とテスト用の [`crate::in_memory::RecordingDomainEventPublisher`]。発行の失敗で
//! ユースケースを失敗させないので、戻り値はない（確実な配送（アウトボックスなど）は基盤を選ぶときに決める）。

use std::{future::Future, time::SystemTime};

use domain::{identity::AccountId, scheduling::BookingId};
use uuid::Uuid;

/// ドメインイベント。変種は AsyncAPI のメッセージに 1 対 1 で対応し、属性はペイロードの `required` と同じ。
/// ペイロードの文字列表現（JSON）は境界のアダプタが決める。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainEvent {
    /// `BookingConfirmed`（scheduling が発行し、notification・analytics が購読する）
    BookingConfirmed {
        /// イベント 1 件ごとの一意な ID（購読側の重複排除に使う）
        event_id: Uuid,
        occurred_at: SystemTime,
        student_id: AccountId,
        coach_id: AccountId,
        booking_id: BookingId,
    },
    /// `BookingCancelled`（scheduling が発行し、entitlement・notification・analytics が購読する）。
    /// 回数を消費するかは scheduling が判定して `consumes_quota` に載せ、消費は entitlement が行う
    /// （docs/adr/0002 Q2）
    BookingCancelled {
        /// イベント 1 件ごとの一意な ID（購読側の重複排除に使う）
        event_id: Uuid,
        occurred_at: SystemTime,
        student_id: AccountId,
        coach_id: AccountId,
        booking_id: BookingId,
        /// キャンセル規定（domain の `CancellationPolicy`）による判定
        consumes_quota: bool,
        cancelled_at: SystemTime,
    },
}

impl DomainEvent {
    /// AsyncAPI のメッセージ名（チャンネルのアドレスと同じ）。
    pub const fn name(&self) -> &'static str {
        match self {
            DomainEvent::BookingConfirmed { .. } => "BookingConfirmed",
            DomainEvent::BookingCancelled { .. } => "BookingCancelled",
        }
    }
}

pub trait DomainEventPublisher: Send + Sync {
    fn publish(&self, event: DomainEvent) -> impl Future<Output = ()> + Send;
}
