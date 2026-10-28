/// 通知の種類。受け取り設定（[`super::NotificationSettings`]）でオン / オフを決める単位で、通知のきっかけ（境界間イベント）
/// ごとに 1 つ。外部での文字列表現は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NotificationKind {
    /// 予約の確定（`BookingConfirmed`）の通知。生徒への確定の知らせと、コーチへの新しい予約の知らせの両方
    BookingConfirmed,
    /// 動画の添削の返却（`VideoReviewReturned`）の通知
    VideoReviewReturned,
    /// チャットのメッセージが届いた（`MessageSent`）ことの通知
    MessageReceived,
}

impl NotificationKind {
    pub const ALL: [NotificationKind; 3] = [
        NotificationKind::BookingConfirmed,
        NotificationKind::VideoReviewReturned,
        NotificationKind::MessageReceived,
    ];
}
