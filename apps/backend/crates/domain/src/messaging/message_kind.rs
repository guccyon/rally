/// メッセージの種類（[`super::MessageContent`] の変種の名前）。外部での文字列表現は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageKind {
    /// 本文（当事者どうしの会話）
    Text,
    /// 添削依頼（生徒が動画を添削依頼として上げた）
    VideoReviewRequest,
    /// 添削の返却（コーチが動画の添削を返した）
    VideoReviewReturn,
    /// スタンプ（当事者が本文の代わりに送った）
    Stamp,
}

impl MessageKind {
    pub const ALL: [MessageKind; 4] = [
        MessageKind::Text,
        MessageKind::VideoReviewRequest,
        MessageKind::VideoReviewReturn,
        MessageKind::Stamp,
    ];

    /// 当事者がチャットとして送る（[`super::MessageThread::send`]）種類か。本文とスタンプは送り、添削依頼・添削の返却は
    /// async-lesson の出来事を messaging が写して足すもので、送らない。
    pub const fn is_chat(self) -> bool {
        match self {
            MessageKind::Text | MessageKind::Stamp => true,
            MessageKind::VideoReviewRequest | MessageKind::VideoReviewReturn => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_text_and_stamps_are_sent_as_chat() {
        assert_eq!(
            MessageKind::ALL.map(MessageKind::is_chat),
            [true, false, false, true]
        );
    }
}
