use crate::DomainError;

/// レッスンの評価の点（[`Rating::MIN`]〜[`Rating::MAX`] の整数）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rating(u8);

impl Rating {
    /// 最低点（student-app の契約の `minimum`）
    pub const MIN: u8 = 1;
    /// 最高点（student-app の契約の `maximum`）
    pub const MAX: u8 = 5;

    /// `value` が 1〜5 でなければ [`DomainError::RatingOutOfRange`]。
    pub fn new(value: i64) -> Result<Self, DomainError> {
        u8::try_from(value)
            .ok()
            .filter(|value| (Self::MIN..=Self::MAX).contains(value))
            .map(Self)
            .ok_or(DomainError::RatingOutOfRange {
                min: Self::MIN,
                max: Self::MAX,
            })
    }

    pub const fn value(self) -> u8 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_one_to_five() {
        for value in 1..=5 {
            assert_eq!(Rating::new(value).unwrap().value(), value as u8);
        }
    }

    #[test]
    fn rejects_values_out_of_range() {
        for value in [0, 6, -1, 256, 257, i64::MAX, i64::MIN] {
            assert_eq!(
                Rating::new(value),
                Err(DomainError::RatingOutOfRange { min: 1, max: 5 }),
                "{value}"
            );
        }
    }
}
