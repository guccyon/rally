use std::time::SystemTime;

use crate::identity::AccountId;

/// 低い評価に本部が付けた「対応済み」の印（glossary の low_rated_review_handling）。評価 1 件に高々 1 つで、付けた本部の
/// アカウントと時刻を持つ。
///
/// 付けるのは [`super::LessonReview::mark_handled`] だけで、すでに印があれば付け直さない（最初の本部のアカウントと時刻を
/// 残す）。外すことはない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LowRatedReviewHandling {
    handled_by: AccountId,
    handled_at: SystemTime,
}

impl LowRatedReviewHandling {
    pub const fn new(handled_by: AccountId, handled_at: SystemTime) -> Self {
        Self {
            handled_by,
            handled_at,
        }
    }

    /// 印を付けた本部のアカウントの ID。
    pub const fn handled_by(&self) -> AccountId {
        self.handled_by
    }

    /// 印を付けた（最初に付けた）時刻。
    pub const fn handled_at(&self) -> SystemTime {
        self.handled_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_account_and_time() {
        let handling = LowRatedReviewHandling::new(AccountId::from_u128(1), SystemTime::UNIX_EPOCH);
        assert_eq!(handling.handled_by(), AccountId::from_u128(1));
        assert_eq!(handling.handled_at(), SystemTime::UNIX_EPOCH);
    }
}
