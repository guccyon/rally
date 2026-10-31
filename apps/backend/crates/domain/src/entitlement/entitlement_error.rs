use std::fmt;

use super::ContractType;
use crate::catalog::Sport;

/// entitlement（受講権と契約）の不変条件の違反。受講権・月額契約・ビデオ通話の暦月の枠・暦月の生成と、枠の消費で
/// 返す。
///
/// 表示名など複数のコンテキストが共有する値の違反は [`crate::DomainError`] に置く。scheduling が予約の確定で
/// 「枠が残っていない」と断るのは scheduling の失敗（[`crate::DomainError::LiveLessonQuotaExceeded`]）で、ここの
/// [`Self::LiveLessonQuotaExceeded`] は受講権の枠を使うときの違反。利用者に見せる文言と HTTP の状態は境界
/// （interface）が変種から決める。[`fmt::Display`] はデバッグ用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntitlementError {
    /// その契約種別ではそのスポーツの契約を結べない（月額はゴルフ以外、ワンショットはゴルフだけ）
    SportNotCoveredByContract {
        contract_type: ContractType,
        sport: Sport,
    },
    /// 暦月の月が 1〜12 でない
    InvalidMonth { month: u8 },
    /// ビデオ通話の暦月の枠で使った回数が契約プランの上限を超える
    LiveLessonsUsedOverLimit { used: u8, limit: u8 },
    /// ビデオ通話の暦月の枠が残っていない（月額で契約プランの上限を超える回。不変条件 1）
    LiveLessonQuotaExceeded,
}

impl fmt::Display for EntitlementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for EntitlementError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_the_debug_form() {
        assert_eq!(
            EntitlementError::LiveLessonQuotaExceeded.to_string(),
            "LiveLessonQuotaExceeded"
        );
        assert_eq!(
            EntitlementError::InvalidMonth { month: 13 }.to_string(),
            "InvalidMonth { month: 13 }"
        );
    }
}
