/// メッセージの種類（[`super::MessageContent`] の変種の名前）。外部での文字列表現は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageKind {
    /// 本文（当事者どうしの会話）
    Text,
    /// 添削依頼（生徒が動画を添削依頼として上げた）
    VideoReviewRequest,
    /// 添削の返却（コーチが動画の添削を返した）
    VideoReviewReturn,
}

impl MessageKind {
    pub const ALL: [MessageKind; 3] = [
        MessageKind::Text,
        MessageKind::VideoReviewRequest,
        MessageKind::VideoReviewReturn,
    ];
}
