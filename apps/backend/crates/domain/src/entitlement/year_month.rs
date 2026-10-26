use crate::DomainError;

/// 暦月（年と月）。ビデオ通話の枠を区切る単位。
///
/// 時刻からどの暦月かを決める（タイムゾーンを含む）のは application（`application::entitlement::calendar_month`）。
/// 外部での文字列表現（契約の `YYYY-MM`）は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct YearMonth {
    year: i32,
    month: u8,
}

impl YearMonth {
    /// `month` は 1〜12。範囲外なら [`DomainError::InvalidMonth`]。
    pub const fn new(year: i32, month: u8) -> Result<Self, DomainError> {
        if month < 1 || month > 12 {
            return Err(DomainError::InvalidMonth { month });
        }
        Ok(Self { year, month })
    }

    pub const fn year(self) -> i32 {
        self.year
    }

    /// 1〜12。
    pub const fn month(self) -> u8 {
        self.month
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_months_1_to_12() {
        for month in 1..=12 {
            let ym = YearMonth::new(2026, month).unwrap();
            assert_eq!((ym.year(), ym.month()), (2026, month));
        }
    }

    #[test]
    fn rejects_months_out_of_range() {
        for month in [0, 13] {
            assert_eq!(
                YearMonth::new(2026, month),
                Err(DomainError::InvalidMonth { month })
            );
        }
    }

    #[test]
    fn orders_by_year_then_month() {
        let dec = YearMonth::new(2026, 12).unwrap();
        let jan = YearMonth::new(2027, 1).unwrap();
        assert!(dec < jan);
        assert!(YearMonth::new(2026, 11).unwrap() < dec);
    }
}
