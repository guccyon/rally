use std::fmt;

/// notification（通知）の不変条件の違反。お知らせと本部からのお知らせの生成と、本部からのお知らせの取り下げと、受け取ったお知らせの組み立てで返す。
///
/// 利用者に見せる文言は境界が変種から決める。[`fmt::Display`] はデバッグ用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationError {
    /// お知らせの既読が作った時刻より前
    NotificationReadBeforeCreated,
    /// 本部からのお知らせの題が空（空白だけを含む）
    EmptyAnnouncementTitle,
    /// 本部からのお知らせの題が長すぎる（`max` は文字数）
    AnnouncementTitleTooLong { max: usize },
    /// 本部からのお知らせの本文が空（空白だけを含む）
    EmptyAnnouncementBody,
    /// 本部からのお知らせの本文が長すぎる（`max` は文字数）
    AnnouncementBodyTooLong { max: usize },
    /// 本部からのお知らせを受け取る生徒がいない（対象のスポーツの生徒が 1 人もいない）
    AnnouncementWithoutRecipients,
    /// 本部からのお知らせはすでに取り下げてある
    AnnouncementAlreadyWithdrawn,
    /// 受け取ったお知らせのお知らせが、組にした本部からのお知らせを指していない
    ReceivedAnnouncementMismatch,
}

impl fmt::Display for NotificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for NotificationError {}
