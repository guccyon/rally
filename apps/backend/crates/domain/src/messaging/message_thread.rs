use std::time::SystemTime;

use super::{
    Message, MessageAuthor, MessageContent, MessageId, MessageKind, MessageSender, MessageThreadId,
    MessagingError,
};
use crate::{
    async_lesson::{VideoId, VideoReviewId},
    identity::AccountId,
};

/// 生徒と担当コーチの 1 対 1 のチャットのスレッド（glossary の message_thread）。集約で、メッセージ
/// （[`Message`]）を送った順に持つ。
///
/// 同じ生徒とコーチのスレッドは 1 つ（保存先が守る）。スレッドを扱えるのは当事者（生徒と担当コーチ）だけ。
///
/// 生徒が送ってよいか（月額は送り放題）は entitlement の利用権が決め、送信（[`Self::send`]）はその答えに従う。
/// 送り放題なので送信の回数は数えない。コーチの送信は生徒の契約に左右されない。
///
/// 添削依頼・添削の返却（[`Self::post_video_review_request`] / [`Self::post_video_review_return`]）は、async-lesson の
/// 出来事を messaging が写して足すメッセージで、チャットの送り放題の判定を通さない（会話ではなくレッスンのログ）。
/// 同じ動画の添削依頼・添削の返却はスレッドに 1 通ずつ。
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
    /// [`MessagingError::MessageThreadWithOneself`]。
    pub fn open(
        id: MessageThreadId,
        student_id: AccountId,
        coach_id: AccountId,
        now: SystemTime,
    ) -> Result<Self, MessagingError> {
        Self::restore(id, student_id, coach_id, now, Vec::new())
    }

    /// 保存先から読んだスレッドを組み立てる（保存先の実装用）。`messages` は送った順。
    ///
    /// 生徒とコーチが同じアカウント（[`MessagingError::MessageThreadWithOneself`]）、メッセージがスレッドを開く前か
    /// 前のメッセージより前に送られている（[`MessagingError::MessageSentBeforeLastActivity`]）なら失敗する。
    pub fn restore(
        id: MessageThreadId,
        student_id: AccountId,
        coach_id: AccountId,
        opened_at: SystemTime,
        messages: Vec<Message>,
    ) -> Result<Self, MessagingError> {
        if student_id == coach_id {
            return Err(MessagingError::MessageThreadWithOneself);
        }
        let mut last_activity_at = opened_at;
        for message in &messages {
            if message.sent_at() < last_activity_at {
                return Err(MessagingError::MessageSentBeforeLastActivity);
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

    /// `account` がスレッドのどちらの当事者か。当事者でなければ [`MessagingError::NotAThreadParticipant`]。
    pub fn participant(&self, account: AccountId) -> Result<MessageSender, MessagingError> {
        if account == self.student_id {
            Ok(MessageSender::Student)
        } else if account == self.coach_id {
            Ok(MessageSender::Coach)
        } else {
            Err(MessagingError::NotAThreadParticipant)
        }
    }

    /// `account` が当事者でなければ [`MessagingError::NotAThreadParticipant`]。
    pub fn ensure_visible_to(&self, account: AccountId) -> Result<(), MessagingError> {
        self.participant(account).map(|_| ())
    }

    /// 当事者 `sender` が `now` にメッセージ `content`（本文かスタンプ）を送る（メッセージの ID は `message_id`）。
    /// 送ったメッセージを返す。
    ///
    /// `student_may_chat` は生徒がチャットを送ってよいかの答え（entitlement の送り放題。月額は `true`）。
    /// 次の不変条件を破れば [`MessagingError`] を返し、スレッドを変えない。
    ///
    /// - 送れるのは当事者だけ（[`MessagingError::NotAThreadParticipant`]）
    /// - 送れるのはチャットの種類（本文・スタンプ。[`MessageKind::is_chat`]）だけ
    ///   （[`MessagingError::MessageKindNotSendable`]）
    /// - 生徒が送れるのは、チャットが送り放題の利用権を持つときだけ（[`MessagingError::ChatNotAllowed`]。
    ///   コーチは問わない）
    /// - 送信はスレッドを開いた時刻と前のメッセージの送信時刻より前にならない（同じ時刻は許す。
    ///   [`MessagingError::MessageSentBeforeLastActivity`]）
    pub fn send(
        &mut self,
        message_id: MessageId,
        sender: AccountId,
        content: MessageContent,
        student_may_chat: bool,
        now: SystemTime,
    ) -> Result<&Message, MessagingError> {
        let sender = self.participant(sender)?;
        if !content.kind().is_chat() {
            return Err(MessagingError::MessageKindNotSendable);
        }
        if sender == MessageSender::Student && !student_may_chat {
            return Err(MessagingError::ChatNotAllowed);
        }
        self.push(message_id, MessageAuthor::Participant(sender), content, now)
    }

    /// 生徒が動画 `video_id` を添削依頼として送ったことを、`now` にスレッドに足す（メッセージの ID は `message_id`。
    /// 送り手はスレッドの生徒）。足したメッセージを返す。同じ動画の添削依頼がすでにあれば何も変えず `None`
    /// （同じ出来事が 2 回届いても 1 通）。
    ///
    /// 送信がスレッドを開いた時刻と前のメッセージの送信時刻より前なら [`MessagingError::MessageSentBeforeLastActivity`]
    /// で、スレッドを変えない。
    pub fn post_video_review_request(
        &mut self,
        message_id: MessageId,
        video_id: VideoId,
        now: SystemTime,
    ) -> Result<Option<&Message>, MessagingError> {
        if self.has(MessageKind::VideoReviewRequest, video_id) {
            return Ok(None);
        }
        self.push(
            message_id,
            MessageAuthor::Participant(MessageSender::Student),
            MessageContent::VideoReviewRequest { video_id },
            now,
        )
        .map(Some)
    }

    /// コーチが動画 `video_id` に添削 `review_id` を返したことを、`now` にスレッドに足す（メッセージの ID は
    /// `message_id`。送り手はスレッドの担当コーチ）。足したメッセージを返す。同じ動画の添削の返却がすでにあれば
    /// 何も変えず `None`（返却は動画 1 本に 1 回。同じ出来事が 2 回届いても 1 通）。
    ///
    /// 添削依頼のメッセージがなくても足す（この機能より前に送られた動画の返却）。送信の時刻の不変条件は
    /// [`Self::post_video_review_request`] と同じ。
    pub fn post_video_review_return(
        &mut self,
        message_id: MessageId,
        video_id: VideoId,
        review_id: VideoReviewId,
        now: SystemTime,
    ) -> Result<Option<&Message>, MessagingError> {
        if self.has(MessageKind::VideoReviewReturn, video_id) {
            return Ok(None);
        }
        self.push(
            message_id,
            MessageAuthor::Participant(MessageSender::Coach),
            MessageContent::VideoReviewReturn {
                video_id,
                review_id,
            },
            now,
        )
        .map(Some)
    }

    /// 種類 `kind` で動画 `video_id` を指すメッセージがあるか。
    fn has(&self, kind: MessageKind, video_id: VideoId) -> bool {
        self.messages.iter().any(|message| {
            message.content().kind() == kind && message.content().video_id() == Some(video_id)
        })
    }

    /// メッセージを `now` に足す。送信がスレッドを開いた時刻と前のメッセージより前なら
    /// [`MessagingError::MessageSentBeforeLastActivity`]。
    fn push(
        &mut self,
        message_id: MessageId,
        author: MessageAuthor,
        content: MessageContent,
        now: SystemTime,
    ) -> Result<&Message, MessagingError> {
        if now < self.last_activity_at() {
            return Err(MessagingError::MessageSentBeforeLastActivity);
        }
        let message = Message::sent(message_id, author, content, now)?;
        self.messages.push(message);
        Ok(self.messages.last().expect("いま足した"))
    }

    /// `message` が当事者 `reader` にとって相手から届いたものか（システムのものは当事者どちらのものでもない）。
    fn is_from_the_other_party(message: &Message, reader: MessageSender) -> bool {
        message
            .author()
            .participant()
            .is_some_and(|sender| sender != reader)
    }

    /// 当事者 `reader` が `now` に、相手から届いたメッセージをすべて既読にする。既読にした数を返す。
    ///
    /// 自分が送ったメッセージと既読のメッセージ、システムのメッセージは変えない（既読は最初に読んだ時刻のまま）。当事者でなければ
    /// [`MessagingError::NotAThreadParticipant`] で、何も変えない。
    pub fn mark_read(
        &mut self,
        reader: AccountId,
        now: SystemTime,
    ) -> Result<usize, MessagingError> {
        let reader = self.participant(reader)?;
        Ok(self
            .messages
            .iter_mut()
            .filter(|message| Self::is_from_the_other_party(message, reader))
            .map(|message| message.read(now))
            .filter(|read| *read)
            .count())
    }

    /// 当事者 `account` にとっての未読（相手から届いてまだ読んでいない）メッセージの数。システムのメッセージは数えない。
    /// 当事者でなければ 0。
    pub fn unread_count_for(&self, account: AccountId) -> usize {
        let Ok(reader) = self.participant(account) else {
            return 0;
        };
        self.messages
            .iter()
            .filter(|message| {
                Self::is_from_the_other_party(message, reader) && message.read_at().is_none()
            })
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
    use crate::messaging::{MessageBody, Stamp};

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

    fn text(text: &str) -> MessageContent {
        MessageContent::Text(body(text))
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
            Err(MessagingError::MessageThreadWithOneself)
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
            Err(MessagingError::NotAThreadParticipant)
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
                text("フォームを見てください"),
                true,
                at(1),
            )
            .unwrap()
            .clone();
        assert_eq!(sent.id(), message(1));
        assert_eq!(
            sent.author(),
            MessageAuthor::Participant(MessageSender::Student)
        );
        assert_eq!(
            sent.content(),
            &MessageContent::Text(body("フォームを見てください"))
        );
        assert_eq!(sent.sent_at(), at(1));
        assert_eq!(sent.read_at(), None);

        // 同じ時刻の送信は許す
        thread
            .send(message(2), COACH, text("了解です"), true, at(1))
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
                .send(message(id), STUDENT, text("質問です"), true, at(1))
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
                text("体験はいかがでしたか"),
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
            .send(message(1), COACH, text("こんにちは"), true, at(5))
            .unwrap();
        for (mut thread, sender, may_chat, now, expected) in [
            (
                opened(),
                OTHER,
                true,
                at(1),
                MessagingError::NotAThreadParticipant,
            ),
            (
                opened(),
                STUDENT,
                false,
                at(1),
                MessagingError::ChatNotAllowed,
            ),
            (
                later,
                STUDENT,
                true,
                at(4),
                MessagingError::MessageSentBeforeLastActivity,
            ),
            (
                MessageThread::open(THREAD, STUDENT, COACH, at(2)).unwrap(),
                COACH,
                true,
                at(1),
                MessagingError::MessageSentBeforeLastActivity,
            ),
        ] {
            let before = thread.clone();
            assert_eq!(
                thread
                    .send(message(9), sender, text("送れない"), may_chat, now)
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
            .send(message(1), STUDENT, text("1"), true, at(1))
            .unwrap();
        thread
            .send(message(2), COACH, text("2"), true, at(2))
            .unwrap();
        thread
            .send(message(3), STUDENT, text("3"), true, at(3))
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
            Err(MessagingError::NotAThreadParticipant)
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
                MessageAuthor::Participant(MessageSender::Student),
                MessageContent::Text(body("x")),
                at(minute),
                None,
            )
            .unwrap()
        };
        assert_eq!(
            MessageThread::restore(THREAD, STUDENT, COACH, NOW, vec![sent(1, 2), sent(2, 1)]),
            Err(MessagingError::MessageSentBeforeLastActivity)
        );
        assert_eq!(
            MessageThread::restore(THREAD, STUDENT, COACH, at(5), vec![sent(1, 4)]),
            Err(MessagingError::MessageSentBeforeLastActivity)
        );
        assert_eq!(
            MessageThread::restore(THREAD, COACH, COACH, NOW, vec![]),
            Err(MessagingError::MessageThreadWithOneself)
        );
        // 開いた時刻と同じ・同じ時刻が並ぶのは読める
        assert!(
            MessageThread::restore(THREAD, STUDENT, COACH, at(1), vec![sent(1, 1), sent(2, 1)])
                .is_ok()
        );
    }

    const VIDEO: VideoId = VideoId::from_u128(40);
    const OTHER_VIDEO: VideoId = VideoId::from_u128(41);
    const REVIEW: VideoReviewId = VideoReviewId::from_u128(50);

    #[test]
    fn a_video_review_request_is_posted_by_the_student_once_per_video() {
        let mut thread = opened();
        let posted = thread
            .post_video_review_request(message(1), VIDEO, at(1))
            .unwrap()
            .cloned()
            .unwrap();
        assert_eq!(
            posted.author(),
            MessageAuthor::Participant(MessageSender::Student)
        );
        assert_eq!(
            posted.content(),
            &MessageContent::VideoReviewRequest { video_id: VIDEO }
        );
        assert_eq!(posted.content().body(), None, "本文は持たない");
        assert_eq!(posted.sent_at(), at(1));
        assert_eq!(thread.unread_count_for(COACH), 1, "コーチに届いた依頼");
        assert_eq!(thread.unread_count_for(STUDENT), 0);

        // 同じ動画の依頼は 2 通目を足さない
        let before = thread.clone();
        assert_eq!(
            thread.post_video_review_request(message(2), VIDEO, at(2)),
            Ok(None)
        );
        assert_eq!(thread, before);
        // 別の動画は足す
        thread
            .post_video_review_request(message(3), OTHER_VIDEO, at(2))
            .unwrap()
            .unwrap();
        assert_eq!(thread.messages().len(), 2);
    }

    #[test]
    fn a_video_review_return_is_posted_by_the_coach_once_per_video() {
        let mut thread = opened();
        thread
            .post_video_review_request(message(1), VIDEO, at(1))
            .unwrap();
        thread
            .send(
                message(2),
                STUDENT,
                text("よろしくお願いします"),
                true,
                at(2),
            )
            .unwrap();
        let posted = thread
            .post_video_review_return(message(3), VIDEO, REVIEW, at(3))
            .unwrap()
            .cloned()
            .unwrap();
        assert_eq!(
            posted.author(),
            MessageAuthor::Participant(MessageSender::Coach)
        );
        assert_eq!(
            posted.content(),
            &MessageContent::VideoReviewReturn {
                video_id: VIDEO,
                review_id: REVIEW
            }
        );
        assert_eq!(thread.unread_count_for(STUDENT), 1, "生徒に届いた返却");
        // 依頼・会話・返却が 1 つの時間軸に並ぶ
        assert_eq!(
            thread
                .messages()
                .iter()
                .map(|m| m.content().kind())
                .collect::<Vec<_>>(),
            [
                MessageKind::VideoReviewRequest,
                MessageKind::Text,
                MessageKind::VideoReviewReturn
            ]
        );

        let before = thread.clone();
        assert_eq!(
            thread.post_video_review_return(message(4), VIDEO, REVIEW, at(4)),
            Ok(None)
        );
        assert_eq!(thread, before);
    }

    #[test]
    fn a_return_is_posted_without_a_request() {
        let mut thread = opened();
        assert!(
            thread
                .post_video_review_return(message(1), VIDEO, REVIEW, at(1))
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn video_review_messages_keep_the_order_of_time() {
        let mut thread = MessageThread::open(THREAD, STUDENT, COACH, at(5)).unwrap();
        let before = thread.clone();
        assert_eq!(
            thread.post_video_review_request(message(1), VIDEO, at(4)),
            Err(MessagingError::MessageSentBeforeLastActivity)
        );
        assert_eq!(
            thread.post_video_review_return(message(1), VIDEO, REVIEW, at(4)),
            Err(MessagingError::MessageSentBeforeLastActivity)
        );
        assert_eq!(thread, before);
    }

    #[test]
    fn system_messages_are_unread_for_no_one_and_are_not_marked_read() {
        let system = Message::restore(
            message(1),
            MessageAuthor::System,
            MessageContent::Text(body("お知らせ")),
            at(1),
            None,
        )
        .unwrap();
        let mut thread =
            MessageThread::restore(THREAD, STUDENT, COACH, NOW, vec![system.clone()]).unwrap();
        assert_eq!(thread.unread_count_for(STUDENT), 0);
        assert_eq!(thread.unread_count_for(COACH), 0);
        assert_eq!(thread.mark_read(STUDENT, at(2)), Ok(0));
        assert_eq!(thread.mark_read(COACH, at(2)), Ok(0));
        assert_eq!(thread.messages(), [system]);
    }

    #[test]
    fn restore_rejects_a_kind_from_the_wrong_author() {
        assert_eq!(
            Message::restore(
                message(1),
                MessageAuthor::Participant(MessageSender::Coach),
                MessageContent::VideoReviewRequest { video_id: VIDEO },
                at(1),
                None
            ),
            Err(MessagingError::MessageAuthorMismatchesKind)
        );
    }

    #[test]
    fn the_participants_send_stamps_like_text() {
        let mut thread = opened();
        let sent = thread
            .send(
                message(1),
                STUDENT,
                MessageContent::Stamp(Stamp::Thanks),
                true,
                at(1),
            )
            .unwrap()
            .clone();
        assert_eq!(
            sent.author(),
            MessageAuthor::Participant(MessageSender::Student)
        );
        assert_eq!(sent.content(), &MessageContent::Stamp(Stamp::Thanks));
        assert_eq!(sent.content().body(), None, "本文は持たない");
        assert_eq!(thread.unread_count_for(COACH), 1, "コーチに届いたスタンプ");

        // コーチは生徒の契約に左右されない
        thread
            .send(
                message(2),
                COACH,
                MessageContent::Stamp(Stamp::NiceShot),
                false,
                at(2),
            )
            .unwrap();
        assert_eq!(thread.unread_count_for(STUDENT), 1);
        assert_eq!(
            thread
                .messages()
                .iter()
                .map(|m| m.content().stamp())
                .collect::<Vec<_>>(),
            [Some(Stamp::Thanks), Some(Stamp::NiceShot)]
        );
    }

    #[test]
    fn a_stamp_needs_the_same_entitlement_as_text() {
        let mut thread = opened();
        assert_eq!(
            thread
                .send(
                    message(1),
                    STUDENT,
                    MessageContent::Stamp(Stamp::Good),
                    false,
                    at(1)
                )
                .map(|_| ()),
            Err(MessagingError::ChatNotAllowed)
        );
        assert!(thread.messages().is_empty());
    }

    #[test]
    fn video_review_messages_are_not_sent_as_chat() {
        let mut thread = opened();
        let before = thread.clone();
        for (sender, content) in [
            (
                STUDENT,
                MessageContent::VideoReviewRequest { video_id: VIDEO },
            ),
            (
                COACH,
                MessageContent::VideoReviewReturn {
                    video_id: VIDEO,
                    review_id: REVIEW,
                },
            ),
        ] {
            assert_eq!(
                thread
                    .send(message(1), sender, content, true, at(1))
                    .map(|_| ()),
                Err(MessagingError::MessageKindNotSendable)
            );
        }
        // 当事者でなければ種類より先に断る（スレッドがあることを知らせない）
        assert_eq!(
            thread
                .send(
                    message(1),
                    OTHER,
                    MessageContent::VideoReviewRequest { video_id: VIDEO },
                    true,
                    at(1)
                )
                .map(|_| ()),
            Err(MessagingError::NotAThreadParticipant)
        );
        assert_eq!(thread, before);
    }
}
