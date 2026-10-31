use std::fmt;

/// notification（通知）の不変条件の違反。お知らせの生成で返す。
///
/// 利用者に見せる文言は境界が変種から決める。[`fmt::Display`] はデバッグ用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationError {
    /// お知らせの既読が作った時刻より前
    NotificationReadBeforeCreated,
}

impl fmt::Display for NotificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for NotificationError {}
