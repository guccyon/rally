use super::EntitlementError;

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
    /// `month` は 1〜12。範囲外なら [`EntitlementError::InvalidMonth`]。
    pub const fn new(year: i32, month: u8) -> Result<Self, EntitlementError> {
        if month < 1 || month > 12 {
            return Err(EntitlementError::InvalidMonth { month });
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

    /// 前の暦月（1 月なら前の年の 12 月）。
    pub const fn previous(self) -> Self {
        if self.month == 1 {
            Self {
                year: self.year - 1,
                month: 12,
            }
        } else {
            Self {
                year: self.year,
                month: self.month - 1,
            }
        }
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
                Err(EntitlementError::InvalidMonth { month })
            );
        }
    }

    #[test]
    fn previous_month_wraps_to_december_of_the_last_year() {
        let ym = |year, month| YearMonth::new(year, month).unwrap();
        assert_eq!(ym(2026, 11).previous(), ym(2026, 10));
        assert_eq!(ym(2027, 1).previous(), ym(2026, 12));
    }

    #[test]
    fn orders_by_year_then_month() {
        let dec = YearMonth::new(2026, 12).unwrap();
        let jan = YearMonth::new(2027, 1).unwrap();
        assert!(dec < jan);
        assert!(YearMonth::new(2026, 11).unwrap() < dec);
    }
}
