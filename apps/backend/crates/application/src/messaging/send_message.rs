use domain::{
    DomainError,
    identity::AccountId,
    messaging::{Message, MessageBody, MessageThread, MessageThreadId},
};

use super::{ChatGate, MessageThreadRepository, UpdateMessageThreadError};
use crate::{
    Unavailable,
    analytics::{AnalyticsEvent, AnalyticsSink},
    clock::Clock,
};

/// メッセージの送信の入力。`sender_id` は認証したアカウント（スレッドの当事者かは domain が確かめる）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendMessageCommand {
    pub sender_id: AccountId,
    pub thread_id: MessageThreadId,
    pub body: MessageBody,
}

/// 送ったメッセージと、送ったあとのスレッド。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentMessage {
    pub thread: MessageThread,
    pub message: Message,
}

#[derive(Debug)]
pub enum SendMessageError {
    /// スレッドがない
    ThreadNotFound,
    /// 送信がドメインの不変条件を満たさない（当事者でない、生徒がチャットの送り放題を持たない、
    /// 前のメッセージより前の送信）
    Invalid(DomainError),
    /// 保存先・問い合わせ先の障害
    Unavailable(Unavailable),
}

/// メッセージの送信のユースケース（student-app・admin-web の `sendMessage`。スレッドの当事者だけ）。
///
/// スレッドを読み、スレッドの生徒がチャットを送ってよいかを entitlement に問い合わせ（[`ChatGate`]。月額は送り
/// 放題で、回数を消費しない）、domain の [`MessageThread::send`] で送って保存する。保存したら行動イベント
/// `message.sent` を 1 回発火する。拒否・失敗では発火しない。
pub struct SendMessage<T, G, A, C> {
    threads: T,
    gate: G,
    analytics: A,
    clock: C,
}

impl<T, G, A, C> SendMessage<T, G, A, C>
where
    T: MessageThreadRepository,
    G: ChatGate,
    A: AnalyticsSink,
    C: Clock,
{
    pub fn new(threads: T, gate: G, analytics: A, clock: C) -> Self {
        Self {
            threads,
            gate,
            analytics,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: SendMessageCommand,
    ) -> Result<SentMessage, SendMessageError> {
        let now = self.clock.now();
        let thread = self
            .threads
            .find(command.thread_id)
            .await
            .map_err(SendMessageError::Unavailable)?
            .ok_or(SendMessageError::ThreadNotFound)?;
        // 当事者でないアカウントには、スレッドの生徒の利用権を問い合わせない
        thread
            .ensure_visible_to(command.sender_id)
            .map_err(SendMessageError::Invalid)?;
        let student_may_chat = self
            .gate
            .may_chat(thread.student_id())
            .await
            .map_err(SendMessageError::Unavailable)?;

        let message_id = self.threads.next_message_id();
        let thread = self
            .threads
            .update(command.thread_id, |thread| {
                thread
                    .send(
                        message_id,
                        command.sender_id,
                        command.body,
                        student_may_chat,
                        now,
                    )
                    .map(|_| ())
            })
            .await
            .map_err(|error| match error {
                UpdateMessageThreadError::NotFound => SendMessageError::ThreadNotFound,
                UpdateMessageThreadError::Rejected(error) => SendMessageError::Invalid(error),
                UpdateMessageThreadError::Unavailable(error) => {
                    SendMessageError::Unavailable(error)
                }
            })?;
        let message = thread
            .messages()
            .iter()
            .find(|message| message.id() == message_id)
            .cloned()
            .expect("送ったメッセージは保存したスレッドにある");

        self.analytics
            .emit(AnalyticsEvent::MessageSent {
                student_id: thread.student_id(),
                coach_id: thread.coach_id(),
                thread_id: thread.id(),
                message_id,
                sender_role: message.sender(),
                occurred_at: now,
            })
            .await;

        Ok(SentMessage { thread, message })
    }

    pub fn threads(&self) -> &T {
        &self.threads
    }

    pub fn analytics(&self) -> &A {
        &self.analytics
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        entitlement::{Entitlement, EntitlementId, LiveLessonQuota, SubscriptionId, YearMonth},
        identity::Sport,
        messaging::MessageSender,
    };

    use super::*;
    use crate::{
        entitlement::EntitlementChatGate,
        in_memory::{
            FixedClock, InMemoryEntitlementRepository, InMemoryMessageThreadRepository,
            RecordingAnalyticsSink,
        },
    };

    const MONTHLY_STUDENT: AccountId = AccountId::from_u128(1);
    const TRIAL_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);
    const MONTHLY_THREAD: MessageThreadId = MessageThreadId::from_u128(10);
    const TRIAL_THREAD: MessageThreadId = MessageThreadId::from_u128(11);

    fn opened_at() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
    }

    fn now() -> SystemTime {
        opened_at() + Duration::from_secs(3600)
    }

    type UseCase = SendMessage<
        InMemoryMessageThreadRepository,
        EntitlementChatGate<InMemoryEntitlementRepository>,
        RecordingAnalyticsSink,
        FixedClock,
    >;

    /// 月額の生徒 1 とコーチ 7 のスレッド 10、受講権のない（体験予約だけの）生徒 2 とコーチ 7 のスレッド 11。
    fn use_case() -> UseCase {
        let entitlements =
            InMemoryEntitlementRepository::with_entitlements([Entitlement::monthly(
                EntitlementId::from_u128(30),
                MONTHLY_STUDENT,
                Sport::Tennis,
                SubscriptionId::from_u128(31),
                LiveLessonQuota::fresh(YearMonth::new(2026, 10).unwrap()),
            )
            .unwrap()]);
        SendMessage::new(
            InMemoryMessageThreadRepository::with_threads([
                MessageThread::open(MONTHLY_THREAD, MONTHLY_STUDENT, COACH, opened_at()).unwrap(),
                MessageThread::open(TRIAL_THREAD, TRIAL_STUDENT, COACH, opened_at()).unwrap(),
            ]),
            EntitlementChatGate::new(entitlements),
            RecordingAnalyticsSink::default(),
            FixedClock(now()),
        )
    }

    fn command(sender_id: AccountId, thread_id: MessageThreadId) -> SendMessageCommand {
        SendMessageCommand {
            sender_id,
            thread_id,
            body: MessageBody::new("次のレッスンの相談です").unwrap(),
        }
    }

    #[tokio::test]
    async fn sending_saves_the_message_and_emits_message_sent() {
        let use_case = use_case();
        let sent = use_case
            .execute(command(MONTHLY_STUDENT, MONTHLY_THREAD))
            .await
            .unwrap();
        assert_eq!(sent.message.sender(), MessageSender::Student);
        assert_eq!(sent.message.sent_at(), now());
        assert_eq!(sent.message.read_at(), None);
        assert_eq!(sent.thread.messages(), std::slice::from_ref(&sent.message));
        assert_eq!(
            use_case
                .threads()
                .all()
                .into_iter()
                .find(|t| t.id() == MONTHLY_THREAD),
            Some(sent.thread.clone())
        );

        let events = use_case.analytics().events();
        assert_eq!(
            events,
            vec![AnalyticsEvent::MessageSent {
                student_id: MONTHLY_STUDENT,
                coach_id: COACH,
                thread_id: MONTHLY_THREAD,
                message_id: sent.message.id(),
                sender_role: MessageSender::Student,
                occurred_at: now(),
            }]
        );
        assert_eq!(events[0].name(), "message.sent");
    }

    #[tokio::test]
    async fn monthly_students_send_as_many_messages_as_they_like() {
        let use_case = use_case();
        for _ in 0..5 {
            use_case
                .execute(command(MONTHLY_STUDENT, MONTHLY_THREAD))
                .await
                .unwrap();
        }
        assert_eq!(use_case.analytics().events().len(), 5);
    }

    #[tokio::test]
    async fn the_coach_replies_even_to_a_student_without_monthly() {
        let use_case = use_case();
        let sent = use_case
            .execute(command(COACH, TRIAL_THREAD))
            .await
            .unwrap();
        assert_eq!(sent.message.sender(), MessageSender::Coach);
        assert!(matches!(
            use_case.analytics().events()[0],
            AnalyticsEvent::MessageSent {
                sender_role: MessageSender::Coach,
                coach_id: COACH,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn rejected_sends_change_nothing_and_emit_nothing() {
        let use_case = use_case();
        let before = use_case.threads().all();
        for (command, expected) in [
            (
                command(TRIAL_STUDENT, TRIAL_THREAD),
                DomainError::ChatNotAllowed,
            ),
            (
                command(OTHER_COACH, MONTHLY_THREAD),
                DomainError::NotAThreadParticipant,
            ),
        ] {
            let result = use_case.execute(command).await;
            assert!(
                matches!(&result, Err(SendMessageError::Invalid(error)) if *error == expected),
                "{result:?}"
            );
        }
        let result = use_case
            .execute(command(COACH, MessageThreadId::from_u128(99)))
            .await;
        assert!(
            matches!(result, Err(SendMessageError::ThreadNotFound)),
            "{result:?}"
        );
        assert_eq!(use_case.threads().all(), before);
        assert!(use_case.analytics().events().is_empty());
    }
}
