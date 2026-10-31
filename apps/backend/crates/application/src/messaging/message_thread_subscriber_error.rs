use std::{error::Error, fmt};

/// messaging の購読（[`super::MessageThreadSubscriber`]）に固有の失敗。
///
/// 不変条件の違反（`MessagingError`）と保存先の障害（[`crate::Unavailable`]）はそれぞれの型のまま返すので、ここには
/// 購読の手順でしか起きない失敗だけを置く。呼び出し側は `downcast_ref` で見分ける。`Display` は診断用で、
/// 利用者向けの文言は表示する境界で組み立てる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageThreadSubscriberError {
    /// 開いた（または既にあった）スレッドに、メッセージを足そうとしたときにはなかった
    ThreadVanished,
}

impl fmt::Display for MessageThreadSubscriberError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ThreadVanished => f.write_str("message thread vanished after it was opened"),
        }
    }
}

impl Error for MessageThreadSubscriberError {}
