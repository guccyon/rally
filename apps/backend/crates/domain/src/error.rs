use std::fmt;

/// ドメインの不変条件の違反。モデルの生成時に返す。
///
/// 利用者に見せる文言は境界（interface）が変種から組み立てる。[`fmt::Display`] はデバッグ用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// 表示名が空（空白だけを含む）
    EmptyDisplayName,
    /// 表示名が長すぎる（`max` は文字数）
    DisplayNameTooLong { max: usize },
    /// メールアドレスの形が不正
    InvalidEmail,
    /// 役割とスポーツの組み合わせが不正（生徒は 1 つ、コーチは 1 つ以上、本部はなし）
    RoleAttributeMismatch {
        role: crate::identity::Role,
        sports: usize,
    },
    /// コーチの自己紹介文が長すぎる（`max` は文字数）
    CoachBioTooLong { max: usize },
    /// スロットの開始が終了より前でない
    SlotEndsBeforeItStarts,
    /// 同じコーチの時間を占める枠（`existing`）と期間が重なる
    SlotOverlaps { existing: crate::scheduling::SlotId },
    /// 別のコーチの枠をコーチのカレンダーに入れようとした
    SlotOfAnotherCoach,
    /// スロットが公開中（`Open`）でないので、予約・確定できない（`status` は今の状態）
    SlotNotOpen {
        status: crate::scheduling::SlotStatus,
    },
    /// スロットが予約済み（`Booked`）でないので、公開中に戻せない（`status` は今の状態）
    SlotNotBooked {
        status: crate::scheduling::SlotStatus,
    },
    /// スロットに有効な予約がすでにある（定員 1。二重予約）
    SlotAlreadyBooked,
    /// 体験予約はその生徒の初回の予約だけ（キャンセルされていない予約がすでにある）
    TrialAlreadyUsed,
    /// 予約が確定前（`Pending`）でない（`status` は今の状態）
    BookingNotPending {
        status: crate::scheduling::BookingStatus,
    },
    /// 予約はすでにキャンセルしてある
    BookingAlreadyCancelled,
    /// 予約した生徒でない
    BookingOfAnotherStudent,
    /// 予約したスロットと別のスロットを渡した
    BookingOfAnotherSlot,
    /// その契約種別ではそのスポーツの契約を結べない（月額はゴルフ以外、ワンショットはゴルフだけ）
    SportNotCoveredByContract {
        contract_type: crate::entitlement::ContractType,
        sport: crate::identity::Sport,
    },
    /// 暦月の月が 1〜12 でない
    InvalidMonth { month: u8 },
    /// ビデオ通話の暦月の枠で使った回数が契約プランの上限を超える
    LiveLessonsUsedOverLimit { used: u8, limit: u8 },
    /// ビデオ通話の暦月の枠が残っていない（月額で契約プランの上限を超える回。不変条件 1）
    LiveLessonQuotaExceeded,
    /// 予約の参加者（生徒・コーチ）でないので、その通話セッションを扱えない
    NotASessionParticipant,
    /// 予約が確定済み（`Confirmed`）でないので、通話を開始できない（`status` は今の状態）
    BookingNotConfirmed {
        status: crate::scheduling::BookingStatus,
    },
    /// 予約の通話セッションはすでにある（同じ予約のセッションは 1 つ）
    LiveLessonSessionAlreadyStarted,
    /// 通話セッションはすでに終了している
    LiveLessonSessionAlreadyEnded,
    /// 通話セッションの終了が開始より前
    SessionEndsBeforeItStarts,
    /// パケット損失率が 0〜100（百分率）の有限の値でない
    InvalidPacketLossRate,
    /// 動画の当事者（送った生徒・送り先のコーチ）でないので、その動画を扱えない
    NotAVideoParty,
    /// 動画の送り先のコーチでないので、添削を返せない
    NotTheVideoCoach,
    /// 動画の添削はすでに返してある（返却は 1 回）
    VideoReviewAlreadyReturned,
    /// その動画の添削にはまだ着手できない（`sport` は動画のスポーツ。ゴルフはワンショット決済の確定が要る。
    /// 不変条件 2）
    VideoReviewNotAllowed { sport: crate::identity::Sport },
    /// 添削の返却が動画の送付より前
    ReviewReturnedBeforeSubmission,
    /// 添削コメントが空（空白だけを含む）
    EmptyReviewComment,
    /// 添削コメントが長すぎる（`max` は文字数）
    ReviewCommentTooLong { max: usize },
    /// 通話セッションがまだ終了していないので、レッスンの完了を記録できない
    LiveLessonSessionNotEnded,
    /// AI の一次フィードバックの文が空（空白だけを含む）
    EmptyAiFeedback,
    /// 動画の AI の一次フィードバックはすでに付けてある（1 本に 1 つ）
    AiFeedbackAlreadyRecorded,
    /// AI の一次フィードバックを作った時刻が動画の送付より前
    AiFeedbackBeforeSubmission,
    /// 動画の添削がまだ返されていないので、レッスンの完了を記録できない
    VideoReviewNotReturned,
    /// レッスンの完了の記録がレッスンの終わり（通話の終了・添削の返却）より前
    CompletionBeforeLessonEnded,
    /// そのレッスン（通話セッション・動画）の完了はすでに記録してある
    LessonAlreadyCompleted,
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
    /// 評価の点が `min`〜`max` の整数でない
    RatingOutOfRange { min: u8, max: u8 },
    /// 評価の一言が空（空白だけを含む）
    EmptyLessonReviewComment,
    /// 評価の一言が長すぎる（`max` は文字数）
    LessonReviewCommentTooLong { max: usize },
    /// 完了したレッスンの生徒でないので、そのレッスンを評価できない
    NotTheLessonStudent,
    /// そのレッスン（完了したレッスン）の評価はすでに付けてある（評価は 1 回）
    LessonAlreadyReviewed,
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for DomainError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_the_debug_form() {
        assert_eq!(DomainError::InvalidEmail.to_string(), "InvalidEmail");
        assert_eq!(
            DomainError::DisplayNameTooLong { max: 50 }.to_string(),
            "DisplayNameTooLong { max: 50 }"
        );
    }
}
