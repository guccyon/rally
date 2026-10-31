use std::fmt;

use super::{BookingStatus, SlotId, SlotStatus};
use crate::catalog::Sport;

/// scheduling（予約と枠）の不変条件の違反。枠・定員・席・コーチのカレンダー・予約の生成と操作と、枠へのレッスン
/// メニューの結びつけと、コーチによる枠の取り下げで返す。
///
/// 表示名など複数のコンテキストが共有する値の違反は [`crate::DomainError`] に置く。予約の確定で entitlement が
/// 「枠が残っていない」と答えたときの拒否は [`Self::LiveLessonQuotaExceeded`]（受講権の枠を使うときの違反は
/// [`crate::entitlement::EntitlementError::LiveLessonQuotaExceeded`]）。利用者に見せる文言と HTTP の状態は境界
/// （interface）が変種から決める。[`fmt::Display`] はデバッグ用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulingError {
    /// スロットの開始が終了より前でない
    SlotEndsBeforeItStarts,
    /// 同じコーチの時間を占める枠（`existing`）と期間が重なる
    SlotOverlaps { existing: SlotId },
    /// 別のコーチの枠をコーチのカレンダーに入れようとした、または別のコーチの枠を取り下げようとした
    SlotOfAnotherCoach,
    /// スロットが公開中（`Open`）でないので、予約・確定・取り下げできない（`status` は今の状態）
    SlotNotOpen { status: SlotStatus },
    /// スロットの定員が `min`〜`max` の整数でない
    SlotCapacityOutOfRange { min: u8, max: u8 },
    /// スロットに確定した予約が入っていない（または閉じた枠な）ので、確定した予約のキャンセルで席を空けられない
    /// （`status` は今の状態）
    SlotNotBooked { status: SlotStatus },
    /// スロットに残りの席がない（有効な予約が定員に達している。定員 1 なら二重予約）
    SlotAlreadyBooked,
    /// スロットの有効な予約（未確定か確定）の数 `active` が定員 `capacity` を超える
    SlotSeatsOverCapacity { capacity: u8, active: u32 },
    /// スロットの状態と席が合わない（予約済み（`Booked`）なら確定した予約が定員ちょうど、公開中（`Open`）なら
    /// 定員未満。`status` は渡した状態）
    SlotStatusMismatchesSeats { status: SlotStatus },
    /// その生徒はそのスロットに有効な予約をすでに持っている（同じ生徒が同じ枠を 2 回は予約できない）
    SlotAlreadyBookedByStudent,
    /// 体験予約はその生徒の初回の予約だけ（キャンセルされていない予約がすでにある）
    TrialAlreadyUsed,
    /// 予約が確定前（`Pending`）でない（`status` は今の状態）
    BookingNotPending { status: BookingStatus },
    /// 予約はすでにキャンセルしてある
    BookingAlreadyCancelled,
    /// 予約した生徒でない
    BookingOfAnotherStudent,
    /// 予約したスロットと別のスロットを渡した
    BookingOfAnotherSlot,
    /// 予約の確定で、entitlement がビデオ通話の暦月の枠が残っていないと答えた（scheduling の拒否。不変条件 1）
    LiveLessonQuotaExceeded,
    /// 別のコーチのレッスンメニューは枠に結びつけられない（枠に結びつけるのは枠のコーチ自身のメニューだけ）
    LessonMenuOfAnotherCoach,
    /// 公開中でないレッスンメニューは枠に結びつけられない
    LessonMenuNotPublished,
    /// 同期レッスンでない（非同期の）レッスンメニューは枠に結びつけられない
    LessonMenuNotLive,
    /// レッスンメニューのスポーツ（`menu`）が枠のスポーツ（`slot`）と違う
    LessonMenuSportMismatch { menu: Sport, slot: Sport },
    /// 枠の長さがレッスンメニューの 1 回の時間（`minutes` 分）と違う
    SlotLengthMismatchesLessonMenu { minutes: u8 },
    /// スロットに有効な予約（未確定か確定。`active` 件）が入っているので取り下げられない
    SlotHasActiveBookings { active: u32 },
    /// スロットはもう始まっている（開始時刻を過ぎた）ので取り下げられない
    SlotAlreadyStarted,
}

impl fmt::Display for SchedulingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for SchedulingError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_the_debug_form() {
        assert_eq!(
            SchedulingError::SlotAlreadyBooked.to_string(),
            "SlotAlreadyBooked"
        );
        assert_eq!(
            SchedulingError::SlotCapacityOutOfRange { min: 1, max: 6 }.to_string(),
            "SlotCapacityOutOfRange { min: 1, max: 6 }"
        );
    }
}
