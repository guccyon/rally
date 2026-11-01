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
    /// 表示名の検索語が長すぎる（`max` は前後の空白を除いた文字数）
    DisplayNameQueryTooLong { max: usize },
    /// メールアドレスの形が不正
    InvalidEmail,
    /// 役割とスポーツの組み合わせが不正（生徒は 1 つ、コーチは 1 つ以上、本部はなし）
    RoleAttributeMismatch {
        role: crate::identity::Role,
        sports: usize,
    },
    /// コーチの自己紹介文が長すぎる（`max` は文字数）
    CoachBioTooLong { max: usize },
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
    VideoReviewNotAllowed { sport: crate::catalog::Sport },
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
    /// レッスンメニューの名前が空（空白だけを含む）
    EmptyLessonMenuName,
    /// レッスンメニューの名前が長すぎる（`max` は文字数）
    LessonMenuNameTooLong { max: usize },
    /// レッスンメニューの説明が長すぎる（`max` は文字数）
    LessonMenuDescriptionTooLong { max: usize },
    /// 同期レッスンの 1 回の時間が `min`〜`max` 分の `step` 分刻みでない
    LessonDurationOutOfRange { min: u8, max: u8, step: u8 },
    /// 1 回の時間がレッスンの種類に合わない（同期は時間が要り、非同期は時間を持てない。`lesson_type` は渡した種類）
    LessonDurationMismatchesType {
        lesson_type: crate::lesson::LessonType,
    },
    /// そのスポーツはコーチの提供スポーツでない（レッスンメニューはコーチのスポーツのどれかで作る）
    SportNotOfferedByCoach { sport: crate::catalog::Sport },
    /// レッスンメニューのコーチでないので、そのメニューを直せない
    NotTheLessonMenuCoach,
    /// 本部がレッスンメニューを非公開にしたときの理由が長すぎる（`max` は文字数）
    HeadquartersUnpublishReasonTooLong { max: usize },
    /// 本部が非公開にしたレッスンメニューは、本部が公開に戻すまでコーチが公開し直せない
    LessonMenuUnpublishedByHeadquarters,
    /// レッスンメニューはすでに本部が非公開にしている
    LessonMenuAlreadyUnpublishedByHeadquarters,
    /// レッスンメニューは本部が非公開にしたものでないので、本部が公開に戻せない
    LessonMenuNotUnpublishedByHeadquarters,
    /// 動画で選べるのは送り先のコーチのレッスンメニューだけ（別のコーチのメニューは選べない）
    VideoLessonMenuOfAnotherCoach,
    /// 公開中でないレッスンメニューは動画で選べない
    VideoLessonMenuNotPublished,
    /// 非同期レッスンでない（同期の）レッスンメニューは動画で選べない
    LessonMenuNotAsync,
    /// レッスンメニューのスポーツ（`menu`）が動画のスポーツ（`video`）と違う
    VideoLessonMenuSportMismatch {
        menu: crate::catalog::Sport,
        video: crate::catalog::Sport,
    },
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
