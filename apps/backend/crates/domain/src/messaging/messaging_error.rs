use std::fmt;

/// messaging（メッセージ）の不変条件の違反。スレッド・メッセージ・本文の生成と操作で返す。
///
/// 表示名など複数のコンテキストが共有する値の違反は [`crate::DomainError`] に置く。利用者に見せる文言と HTTP の
/// 状態は境界（interface）が変種から決める。[`fmt::Display`] はデバッグ用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessagingError {
    /// スレッドの生徒とコーチが同じアカウント
    MessageThreadWithOneself,
    /// スレッドの当事者（生徒・担当コーチ）でないので、そのスレッドを扱えない
    NotAThreadParticipant,
    /// 生徒がチャットを送れる利用権（月額の送り放題）を持たない
    ChatNotAllowed,
    /// メッセージの送信がスレッドを開いた時刻か前のメッセージの送信時刻より前
    MessageSentBeforeLastActivity,
    /// メッセージの既読が送信より前
    MessageReadBeforeSent,
    /// メッセージの本文が空（空白だけを含む）
    EmptyMessageBody,
    /// メッセージの本文が長すぎる（`max` は文字数）
    MessageBodyTooLong { max: usize },
    /// メッセージの送り手がメッセージの種類に合わない（添削依頼は生徒、添削の返却はコーチ、スタンプは当事者だけが送り手になる）
    MessageAuthorMismatchesKind,
    /// 当事者がチャットとして送れない種類のメッセージ（添削依頼・添削の返却は async-lesson の出来事から足すもの）
    MessageKindNotSendable,
    /// 本部が見る、返信のないまま時間の過ぎたスレッドの生徒の最後の発言からの時間が `min`〜`max` 時間でない
    UnansweredThreadHoursOutOfRange { min: u16, max: u16 },
}

impl fmt::Display for MessagingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for MessagingError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_the_debug_form() {
        assert_eq!(
            MessagingError::NotAThreadParticipant.to_string(),
            "NotAThreadParticipant"
        );
        assert_eq!(
            MessagingError::MessageBodyTooLong { max: 2000 }.to_string(),
            "MessageBodyTooLong { max: 2000 }"
        );
    }
}
