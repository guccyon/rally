use super::MessageSender;

/// メッセージをスレッドに流したのが誰か（送り手）。スレッドの当事者（[`MessageSender`]）か、システムか。
///
/// 当事者のメッセージは相手にとっての未読・既読を持つ。システムが流したもの（お知らせなど。まだ流す処理はない）は
/// どちらの当事者のものでもないので、未読に数えず既読にもしない。外部での文字列表現は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageAuthor {
    /// スレッドの当事者（生徒か担当コーチ）
    Participant(MessageSender),
    /// システム（当事者のどちらでもない）
    System,
}

impl MessageAuthor {
    pub const ALL: [MessageAuthor; 3] = [
        MessageAuthor::Participant(MessageSender::Student),
        MessageAuthor::Participant(MessageSender::Coach),
        MessageAuthor::System,
    ];

    /// 当事者ならどちらの当事者か。システムなら `None`。
    pub const fn participant(self) -> Option<MessageSender> {
        match self {
            MessageAuthor::Participant(sender) => Some(sender),
            MessageAuthor::System => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_participants_have_a_side() {
        assert_eq!(
            MessageAuthor::Participant(MessageSender::Coach).participant(),
            Some(MessageSender::Coach)
        );
        assert_eq!(MessageAuthor::System.participant(), None);
    }
}
