use std::time::SystemTime;

use crate::SlotPlan;

/// 生徒 1 人の履歴の 1 歩。`at` に時計を合わせて、対応するユースケースを 1 回呼ぶ。
///
/// 予約・通話・動画は「その生徒の直近のもの」を指す（同じ生徒の予約の連なり・動画の連なりは時間が重ならない
/// ように [`crate::SeedPlan`] が作る）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// アカウントの登録（`RegisterAccount`）
    Register { at: SystemTime },
    /// 生徒のコーチが枠を公開する（`PublishSlot`）
    PublishSlot { at: SystemTime, slot: SlotPlan },
    /// 直近に公開した枠を予約する（`CreateBooking`）
    Book { at: SystemTime, is_trial: bool },
    /// 直近の予約を確定する（`ConfirmBooking`）
    Confirm { at: SystemTime },
    /// 直近の予約をキャンセルする（`CancelBooking`。24 時間を切るかの判定は scheduling）
    Cancel { at: SystemTime },
    /// 直近の予約の通話を生徒が始める（`StartLiveLessonSession`）
    StartCall { at: SystemTime },
    /// 直近の通話をコーチが終える（`EndLiveLessonSession`）
    EndCall { at: SystemTime },
    /// 直近の通話の完了を記録する（`RecordLessonCompletion`）
    CompleteCall { at: SystemTime, by_coach: bool },
    /// 月額契約を始める（`StartSubscription`）
    Subscribe { at: SystemTime },
    /// 生徒のコーチに動画を送る（`SubmitVideo`）
    SubmitVideo { at: SystemTime },
    /// 直近の動画にコーチが添削を返す（`ReturnVideoReview`）
    ReturnReview { at: SystemTime },
    /// 直近の動画の完了を記録する（`RecordLessonCompletion`）
    CompleteVideo { at: SystemTime, by_coach: bool },
}

impl Step {
    /// 時計を合わせる時刻。
    pub const fn at(&self) -> SystemTime {
        match *self {
            Step::Register { at }
            | Step::PublishSlot { at, .. }
            | Step::Book { at, .. }
            | Step::Confirm { at }
            | Step::Cancel { at }
            | Step::StartCall { at }
            | Step::EndCall { at }
            | Step::CompleteCall { at, .. }
            | Step::Subscribe { at }
            | Step::SubmitVideo { at }
            | Step::ReturnReview { at }
            | Step::CompleteVideo { at, .. } => at,
        }
    }
}
