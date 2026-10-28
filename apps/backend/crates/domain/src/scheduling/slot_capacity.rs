use crate::DomainError;

/// スロットの定員（1 つの枠に入れる有効な予約の数。[`SlotCapacity::MIN`]〜[`SlotCapacity::MAX`] の整数）。
///
/// 1 はマンツーマン、2 以上はグループレッスン（変更シナリオ cs-004）。公開で省いたときの既定は 1
/// （[`SlotCapacity::default`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SlotCapacity(u8);

impl SlotCapacity {
    /// 最小の定員（admin-web の契約の `minimum`）
    pub const MIN: u8 = 1;
    /// 最大の定員（admin-web の契約の `maximum`）
    pub const MAX: u8 = 6;
    /// マンツーマン（定員 1）
    pub const ONE: Self = Self(1);

    /// `value` が 1〜6 でなければ [`DomainError::SlotCapacityOutOfRange`]。
    pub fn new(value: i64) -> Result<Self, DomainError> {
        u8::try_from(value)
            .ok()
            .filter(|value| (Self::MIN..=Self::MAX).contains(value))
            .map(Self)
            .ok_or(DomainError::SlotCapacityOutOfRange {
                min: Self::MIN,
                max: Self::MAX,
            })
    }

    pub const fn value(self) -> u8 {
        self.0
    }
}

impl Default for SlotCapacity {
    /// 公開で定員を省いたときの定員（1。マンツーマン）。
    fn default() -> Self {
        Self::ONE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_one_to_six_and_defaults_to_one() {
        for value in 1..=6 {
            assert_eq!(SlotCapacity::new(value).unwrap().value(), value as u8);
        }
        assert_eq!(SlotCapacity::default(), SlotCapacity::ONE);
        assert_eq!(SlotCapacity::default().value(), 1);
    }

    #[test]
    fn rejects_values_out_of_range() {
        for value in [0, 7, -1, 256, 257, i64::MAX, i64::MIN] {
            assert_eq!(
                SlotCapacity::new(value),
                Err(DomainError::SlotCapacityOutOfRange { min: 1, max: 6 }),
                "{value}"
            );
        }
    }
}
