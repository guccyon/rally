use std::time::SystemTime;

use super::{Message, MessageBody, MessageId, MessageSender, MessageThreadId};
use crate::{DomainError, identity::AccountId};

/// 生徒と担当コーチの 1 対 1 のチャットのスレッド（glossary の message_thread）。集約で、メッセージ
/// （[`Message`]）を送った順に持つ。
///
/// 同じ生徒とコーチのスレッドは 1 つ（保存先が守る）。スレッドを扱えるのは当事者（生徒と担当コーチ）だけ。
///
/// 生徒が送ってよいか（月額は送り放題）は entitlement の利用権が決め、送信（[`Self::send`]）はその答えに従う。
/// 送り放題なので送信の回数は数えない。コーチの送信は生徒の契約に左右されない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageThread {
    id: MessageThreadId,
    student_id: AccountId,
    coach_id: AccountId,
    opened_at: SystemTime,
    messages: Vec<Message>,
}

impl MessageThread {
    /// 生徒 `student_id` と担当コーチ `coach_id` のスレッドを `now` に開く。生徒とコーチが同じアカウントなら
    /// [`DomainError::MessageThreadWithOneself`]。
    pub fn open(
        id: MessageThreadId,
        student_id: AccountId,
        coach_id: AccountId,
        now: SystemTime,
    ) -> Result<Self, DomainError> {
        Self::restore(id, student_id, coach_id, now, Vec::new())
    }

    /// 保存先から読んだスレッドを組み立てる（保存先の実装用）。`messages` は送った順。
    ///
    /// 生徒とコーチが同じアカウント（[`DomainError::MessageThreadWithOneself`]）、メッセージがスレッドを開く前か
    /// 前のメッセージより前に送られている（[`DomainError::MessageSentBeforeLastActivity`]）なら失敗する。
    pub fn restore(
        id: MessageThreadId,
        student_id: AccountId,
        coach_id: AccountId,
        opened_at: SystemTime,
        messages: Vec<Message>,
    ) -> Result<Self, DomainError> {
        if student_id == coach_id {
            return Err(DomainError::MessageThreadWithOneself);
        }
        let mut last_activity_at = opened_at;
        for message in &messages {
            if message.sent_at() < last_activity_at {
                return Err(DomainError::MessageSentBeforeLastActivity);
            }
            last_activity_at = message.sent_at();
        }
        Ok(Self {
            id,
            student_id,
            coach_id,
            opened_at,
            messages,
        })
    }

    /// `account` がスレッドのどちらの当事者か。当事者でなければ [`DomainError::NotAThreadParticipant`]。
    pub fn participant(&self, account: AccountId) -> Result<MessageSender, DomainError> {
        if account == self.student_id {
            Ok(MessageSender::Student)
        } else if account == self.coach_id {
            Ok(MessageSender::Coach)
        } else {
            Err(DomainError::NotAThreadParticipant)
        }
    }

    /// `account` が当事者でなければ [`DomainError::NotAThreadParticipant`]。
    pub fn ensure_visible_to(&self, account: AccountId) -> Result<(), DomainError> {
        self.participant(account).map(|_| ())
    }

    /// 当事者 `sender` が `now` にメッセージ `body` を送る（メッセージの ID は `message_id`）。送ったメッセージを返す。
    ///
    /// `student_may_chat` は生徒がチャットを送ってよいかの答え（entitlement の送り放題。月額は `true`）。
    /// 次の不変条件を破れば [`DomainError`] を返し、スレッドを変えない。
    ///
    /// - 送れるのは当事者だけ（[`DomainError::NotAThreadParticipant`]）
    /// - 生徒が送れるのは、チャットが送り放題の利用権を持つときだけ（[`DomainError::ChatNotAllowed`]。
    ///   コーチは問わない）
    /// - 送信はスレッドを開いた時刻と前のメッセージの送信時刻より前にならない（同じ時刻は許す。
    ///   [`DomainError::MessageSentBeforeLastActivity`]）
    pub fn send(
        &mut self,
        message_id: MessageId,
        sender: AccountId,
        body: MessageBody,
        student_may_chat: bool,
        now: SystemTime,
    ) -> Result<&Message, DomainError> {
        let sender = self.participant(sender)?;
        if sender == MessageSender::Student && !student_may_chat {
            return Err(DomainError::ChatNotAllowed);
        }
        if now < self.last_activity_at() {
            return Err(DomainError::MessageSentBeforeLastActivity);
        }
        self.messages
            .push(Message::sent(message_id, sender, body, now));
        Ok(self.messages.last().expect("いま足した"))
    }

    /// 当事者 `reader` が `now` に、相手から届いたメッセージをすべて既読にする。既読にした数を返す。
    ///
    /// 自分が送ったメッセージと既読のメッセージは変えない（最初に読んだ時刻のまま）。当事者でなければ
    /// [`DomainError::NotAThreadParticipant`] で、何も変えない。
    pub fn mark_read(&mut self, reader: AccountId, now: SystemTime) -> Result<usize, DomainError> {
        let reader = self.participant(reader)?;
        Ok(self
            .messages
            .iter_mut()
            .filter(|message| message.sender() != reader)
            .map(|message| message.read(now))
            .filter(|read| *read)
            .count())
    }

    /// 当事者 `account` にとっての未読（相手から届いてまだ読んでいない）メッセージの数。当事者でなければ 0。
    pub fn unread_count_for(&self, account: AccountId) -> usize {
        let Ok(reader) = self.participant(account) else {
            return 0;
        };
        self.messages
            .iter()
            .filter(|message| message.sender() != reader && message.read_at().is_none())
            .count()
    }

    /// 最後の出来事の時刻（最後のメッセージの送信時刻。メッセージがなければスレッドを開いた時刻）。
    pub fn last_activity_at(&self) -> SystemTime {
        self.messages
            .last()
            .map_or(self.opened_at, Message::sent_at)
    }

    /// 当事者 `sender` のアカウント。
    pub const fn account_of(&self, sender: MessageSender) -> AccountId {
        match sender {
            MessageSender::Student => self.student_id,
            MessageSender::Coach => self.coach_id,
        }
    }

    pub const fn id(&self) -> MessageThreadId {
        self.id
    }

    pub const fn student_id(&self) -> AccountId {
        self.student_id
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    pub const fn opened_at(&self) -> SystemTime {
        self.opened_at
    }

    /// メッセージ（送った順）。
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// 最後のメッセージ。まだなければ `None`。
    pub fn last_message(&self) -> Option<&Message> {
        self.messages.last()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(2);
    const OTHER: AccountId = AccountId::from_u128(3);
    const THREAD: MessageThreadId = MessageThreadId::from_u128(10);
    const NOW: SystemTime = SystemTime::UNIX_EPOCH;

    fn at(minute: u64) -> SystemTime {
        NOW + Duration::from_secs(minute * 60)
    }

    fn body(text: &str) -> MessageBody {
        MessageBody::new(text).unwrap()
    }

    fn opened() -> MessageThread {
        MessageThread::open(THREAD, STUDENT, COACH, NOW).unwrap()
    }

    fn message(id: u128) -> MessageId {
        MessageId::from_u128(id)
    }

    #[test]
    fn an_opened_thread_has_no_messages() {
        let thread = opened();
        assert_eq!(thread.id(), THREAD);
        assert_eq!((thread.student_id(), thread.coach_id()), (STUDENT, COACH));
        assert_eq!(thread.opened_at(), NOW);
        assert!(thread.messages().is_empty());
        assert_eq!(thread.last_message(), None);
        assert_eq!(thread.last_activity_at(), NOW);
        assert_eq!(thread.unread_count_for(STUDENT), 0);
    }

    #[test]
    fn a_student_cannot_open_a_thread_with_oneself() {
        assert_eq!(
            MessageThread::open(THREAD, STUDENT, STUDENT, NOW),
            Err(DomainError::MessageThreadWithOneself)
        );
    }

    #[test]
    fn only_the_participants_can_see_a_thread() {
        let thread = opened();
        assert_eq!(thread.participant(STUDENT), Ok(MessageSender::Student));
        assert_eq!(thread.participant(COACH), Ok(MessageSender::Coach));
        assert_eq!(thread.ensure_visible_to(STUDENT), Ok(()));
        assert_eq!(
            thread.ensure_visible_to(OTHER),
            Err(DomainError::NotAThreadParticipant)
        );
        assert_eq!(thread.account_of(MessageSender::Student), STUDENT);
        assert_eq!(thread.account_of(MessageSender::Coach), COACH);
    }

    #[test]
    fn the_participants_exchange_messages_in_order() {
        let mut thread = opened();
        let sent = thread
            .send(
                message(1),
                STUDENT,
                body("フォームを見てください"),
                true,
                at(1),
            )
            .unwrap()
            .clone();
        assert_eq!(sent.id(), message(1));
        assert_eq!(sent.sender(), MessageSender::Student);
        assert_eq!(sent.body(), &body("フォームを見てください"));
        assert_eq!(sent.sent_at(), at(1));
        assert_eq!(sent.read_at(), None);

        // 同じ時刻の送信は許す
        thread
            .send(message(2), COACH, body("了解です"), true, at(1))
            .unwrap();
        assert_eq!(
            thread
                .messages()
                .iter()
                .map(Message::id)
                .collect::<Vec<_>>(),
            [message(1), message(2)]
        );
        assert_eq!(thread.last_message().map(Message::id), Some(message(2)));
        assert_eq!(thread.last_activity_at(), at(1));
    }

    #[test]
    fn monthly_students_can_send_as_many_messages_as_they_like() {
        let mut thread = opened();
        for id in 1..=50 {
            thread
                .send(message(id), STUDENT, body("質問です"), true, at(1))
                .unwrap();
        }
        assert_eq!(thread.messages().len(), 50);
    }

    #[test]
    fn the_coach_can_send_whatever_the_contract_of_the_student() {
        let mut thread = opened();
        thread
            .send(
                message(1),
                COACH,
                body("体験はいかがでしたか"),
                false,
                at(1),
            )
            .unwrap();
        assert_eq!(thread.messages().len(), 1);
    }

    #[test]
    fn send_failures_change_nothing() {
        let mut later = opened();
        later
            .send(message(1), COACH, body("こんにちは"), true, at(5))
            .unwrap();
        for (mut thread, sender, may_chat, now, expected) in [
            (
                opened(),
                OTHER,
                true,
                at(1),
                DomainError::NotAThreadParticipant,
            ),
            (opened(), STUDENT, false, at(1), DomainError::ChatNotAllowed),
            (
                later,
                STUDENT,
                true,
                at(4),
                DomainError::MessageSentBeforeLastActivity,
            ),
            (
                MessageThread::open(THREAD, STUDENT, COACH, at(2)).unwrap(),
                COACH,
                true,
                at(1),
                DomainError::MessageSentBeforeLastActivity,
            ),
        ] {
            let before = thread.clone();
            assert_eq!(
                thread
                    .send(message(9), sender, body("送れない"), may_chat, now)
                    .map(|_| ()),
                Err(expected)
            );
            assert_eq!(thread, before);
        }
    }

    /// 生徒 → コーチ → 生徒の順に 1 通ずつ送ったスレッド。
    fn conversation() -> MessageThread {
        let mut thread = opened();
        thread
            .send(message(1), STUDENT, body("1"), true, at(1))
            .unwrap();
        thread
            .send(message(2), COACH, body("2"), true, at(2))
            .unwrap();
        thread
            .send(message(3), STUDENT, body("3"), true, at(3))
            .unwrap();
        thread
    }

    #[test]
    fn marking_read_reads_only_the_messages_from_the_other_party() {
        let mut thread = conversation();
        assert_eq!(thread.unread_count_for(COACH), 2);
        assert_eq!(thread.unread_count_for(STUDENT), 1);
        assert_eq!(thread.unread_count_for(OTHER), 0);

        assert_eq!(thread.mark_read(COACH, at(10)), Ok(2));
        assert_eq!(thread.unread_count_for(COACH), 0);
        assert_eq!(
            thread.unread_count_for(STUDENT),
            1,
            "生徒の未読は変わらない"
        );
        let read_at: Vec<_> = thread.messages().iter().map(Message::read_at).collect();
        assert_eq!(read_at, [Some(at(10)), None, Some(at(10))]);

        // 2 回目は何も変えず、最初に読んだ時刻のまま
        assert_eq!(thread.mark_read(COACH, at(20)), Ok(0));
        assert_eq!(thread.messages()[0].read_at(), Some(at(10)));

        assert_eq!(thread.mark_read(STUDENT, at(11)), Ok(1));
        assert_eq!(thread.messages()[1].read_at(), Some(at(11)));
    }

    #[test]
    fn marking_read_does_not_read_messages_sent_later() {
        let mut thread = conversation();
        assert_eq!(thread.mark_read(COACH, at(2)), Ok(1));
        assert_eq!(thread.unread_count_for(COACH), 1);
    }

    #[test]
    fn only_the_participants_can_mark_read() {
        let mut thread = conversation();
        let before = thread.clone();
        assert_eq!(
            thread.mark_read(OTHER, at(10)),
            Err(DomainError::NotAThreadParticipant)
        );
        assert_eq!(thread, before);
    }

    #[test]
    fn restore_round_trips_a_conversation() {
        let mut thread = conversation();
        thread.mark_read(COACH, at(10)).unwrap();
        let restored =
            MessageThread::restore(THREAD, STUDENT, COACH, NOW, thread.messages().to_vec())
                .unwrap();
        assert_eq!(restored, thread);
    }

    #[test]
    fn restore_rejects_messages_out_of_order_and_threads_with_oneself() {
        let sent = |id, minute| {
            Message::restore(
                message(id),
                MessageSender::Student,
                body("x"),
                at(minute),
                None,
            )
            .unwrap()
        };
        assert_eq!(
            MessageThread::restore(THREAD, STUDENT, COACH, NOW, vec![sent(1, 2), sent(2, 1)]),
            Err(DomainError::MessageSentBeforeLastActivity)
        );
        assert_eq!(
            MessageThread::restore(THREAD, STUDENT, COACH, at(5), vec![sent(1, 4)]),
            Err(DomainError::MessageSentBeforeLastActivity)
        );
        assert_eq!(
            MessageThread::restore(THREAD, COACH, COACH, NOW, vec![]),
            Err(DomainError::MessageThreadWithOneself)
        );
        // 開いた時刻と同じ・同じ時刻が並ぶのは読める
        assert!(
            MessageThread::restore(THREAD, STUDENT, COACH, at(1), vec![sent(1, 1), sent(2, 1)])
                .is_ok()
        );
    }
}
