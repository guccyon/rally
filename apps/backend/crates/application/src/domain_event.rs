//! 境界づけられたコンテキスト間のドメインイベント（specs/contracts/events/asyncapi.yaml）を発行するポート。
//!
//! 行動イベント（[`crate::analytics`]）とは別物。Gen1 のメッセージ基盤は選定しておらず、実装は infra の
//! モック（ログ出力・購読側に渡すインメモリのイベントバス）とテスト用の [`crate::in_memory::RecordingDomainEventPublisher`]。発行の失敗で
//! ユースケースを失敗させないので、戻り値はない（確実な配送（アウトボックスなど）は基盤を選ぶときに決める）。

use std::{future::Future, time::SystemTime};

use domain::{
    async_lesson::{VideoId, VideoReviewId},
    entitlement::SubscriptionId,
    identity::AccountId,
    lesson::LessonCompletionId,
    messaging::{MessageId, MessageSender, MessageThreadId},
    scheduling::BookingId,
};
use uuid::Uuid;

/// ドメインイベント。変種は AsyncAPI のメッセージに 1 対 1 で対応し、属性はペイロードの `required` と同じ。
/// ペイロードの文字列表現（JSON）は境界のアダプタが決める。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainEvent {
    /// `BookingConfirmed`（scheduling が発行し、entitlement・notification・analytics が購読する）。
    /// entitlement は確定した予約でビデオ通話の暦月の枠を使う
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
    /// `LessonCompleted`（live-lesson・async-lesson が発行し、billing・review・analytics が購読する）。
    /// レッスン（通話セッション・添削を返した動画）の完了が記録された
    LessonCompleted {
        /// イベント 1 件ごとの一意な ID（購読側の重複排除に使う）
        event_id: Uuid,
        occurred_at: SystemTime,
        student_id: AccountId,
        coach_id: AccountId,
        lesson_id: LessonCompletionId,
    },
    /// `SubscriptionStarted`（entitlement が発行し、billing・analytics が購読する）
    SubscriptionStarted {
        /// イベント 1 件ごとの一意な ID（購読側の重複排除に使う）
        event_id: Uuid,
        occurred_at: SystemTime,
        student_id: AccountId,
        subscription_id: SubscriptionId,
    },
    /// `VideoReviewRequested`（async-lesson が発行し、messaging が購読する）。
    /// 生徒が動画を添削依頼として送った。messaging はその生徒とコーチのスレッドに添削依頼のメッセージを足す
    VideoReviewRequested {
        /// イベント 1 件ごとの一意な ID（購読側の重複排除に使う）
        event_id: Uuid,
        occurred_at: SystemTime,
        student_id: AccountId,
        coach_id: AccountId,
        video_id: VideoId,
    },
    /// `VideoReviewReturned`（async-lesson が発行し、notification・messaging が購読する）。
    /// コーチが動画の添削を返却した。notification は動画を送った生徒に知らせ、messaging はその生徒とコーチの
    /// スレッドに添削の返却のメッセージを足す
    VideoReviewReturned {
        /// イベント 1 件ごとの一意な ID（購読側の重複排除に使う）
        event_id: Uuid,
        occurred_at: SystemTime,
        student_id: AccountId,
        coach_id: AccountId,
        video_id: VideoId,
        review_id: VideoReviewId,
    },
    /// `MessageSent`（messaging が発行し、notification が購読する）。
    /// チャットのスレッドでメッセージが送られた。notification は受け手（送り手でない側の当事者）に知らせる
    MessageSent {
        /// イベント 1 件ごとの一意な ID（購読側の重複排除に使う）
        event_id: Uuid,
        occurred_at: SystemTime,
        student_id: AccountId,
        coach_id: AccountId,
        thread_id: MessageThreadId,
        message_id: MessageId,
        /// 送り手がスレッドのどちらの当事者か
        sender_role: MessageSender,
    },
}

impl DomainEvent {
    /// AsyncAPI のメッセージ名（チャンネルのアドレスと同じ）。
    pub const fn name(&self) -> &'static str {
        match self {
            DomainEvent::BookingConfirmed { .. } => "BookingConfirmed",
            DomainEvent::BookingCancelled { .. } => "BookingCancelled",
            DomainEvent::LessonCompleted { .. } => "LessonCompleted",
            DomainEvent::SubscriptionStarted { .. } => "SubscriptionStarted",
            DomainEvent::VideoReviewRequested { .. } => "VideoReviewRequested",
            DomainEvent::VideoReviewReturned { .. } => "VideoReviewReturned",
            DomainEvent::MessageSent { .. } => "MessageSent",
        }
    }
}

pub trait DomainEventPublisher: Send + Sync {
    fn publish(&self, event: DomainEvent) -> impl Future<Output = ()> + Send;
}

/// 境界間イベントの購読側のポート。購読するコンテキストが実装し、メッセージ基盤（Gen1 は infra のインメモリの
/// イベントバスのモック）が発行されたイベントを渡す。関係のないイベントは何もせずに `Ok` を返す。
///
/// 失敗（保存先の障害、不変条件の違反）は `Err` で返し、発行元には伝えない（発行の失敗でユースケースを
/// 失敗させないのと同じ。基盤が記録する）。同じイベントが 2 回届いても結果が変わらないように実装する。
pub trait DomainEventSubscriber: Send + Sync {
    fn handle(
        &self,
        event: &DomainEvent,
    ) -> impl Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send;
}

/// 2 つの購読側に同じイベントを順に渡す（メッセージ基盤のモックが 1 つの購読側しか持たないので、複数の
/// コンテキストの購読をまとめる）。片方が失敗しても、もう片方には渡す。失敗はどちらかの `Err` を返す
/// （両方失敗すれば最初のもの）。
impl<A, B> DomainEventSubscriber for (A, B)
where
    A: DomainEventSubscriber,
    B: DomainEventSubscriber,
{
    async fn handle(
        &self,
        event: &DomainEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let first = self.0.handle(event).await;
        let second = self.1.handle(event).await;
        first.and(second)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    /// 受け取ったイベントの名前を記録し、`fail` なら失敗する購読側。
    #[derive(Clone, Default)]
    struct Recording {
        seen: Arc<Mutex<Vec<&'static str>>>,
        fail: bool,
    }

    impl DomainEventSubscriber for Recording {
        async fn handle(
            &self,
            event: &DomainEvent,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.seen.lock().unwrap().push(event.name());
            if self.fail {
                Err("failed".into())
            } else {
                Ok(())
            }
        }
    }

    fn event() -> DomainEvent {
        DomainEvent::SubscriptionStarted {
            event_id: Uuid::nil(),
            occurred_at: SystemTime::UNIX_EPOCH,
            student_id: AccountId::from_u128(1),
            subscription_id: SubscriptionId::from_u128(2),
        }
    }

    #[tokio::test]
    async fn a_pair_passes_the_event_to_both_even_if_the_first_fails() {
        let failing = Recording {
            fail: true,
            ..Recording::default()
        };
        let ok = Recording::default();
        let pair = (failing.clone(), ok.clone());
        assert!(pair.handle(&event()).await.is_err());
        assert_eq!(*failing.seen.lock().unwrap(), ["SubscriptionStarted"]);
        assert_eq!(*ok.seen.lock().unwrap(), ["SubscriptionStarted"]);

        let both_ok = (Recording::default(), Recording::default());
        assert!(both_ok.handle(&event()).await.is_ok());
    }
}
