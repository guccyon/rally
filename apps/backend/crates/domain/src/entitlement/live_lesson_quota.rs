use super::YearMonth;
use crate::DomainError;

/// ビデオ通話の暦月の枠（月額のみ）。月額契約の生徒のビデオ通話は暦月 2 回まで（不変条件 1）。
///
/// 上限（[`Self::LIMIT`]）はここ 1 か所だけに置く。月をまたいだら使った回数は 0 から数える（[`Self::in_month`]）。
/// 枠の消費（T11）もこの型に足す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveLessonQuota {
    month: YearMonth,
    used: u8,
}

impl LiveLessonQuota {
    /// 暦月あたりのビデオ通話の上限。
    const LIMIT: u8 = 2;

    /// `month` の、まだ使っていない枠。
    pub const fn fresh(month: YearMonth) -> Self {
        Self { month, used: 0 }
    }

    /// 保存してある枠を戻す。`used` が上限を超えるなら [`DomainError::LiveLessonsUsedOverLimit`]。
    pub const fn restore(month: YearMonth, used: u8) -> Result<Self, DomainError> {
        if used > Self::LIMIT {
            return Err(DomainError::LiveLessonsUsedOverLimit {
                used,
                limit: Self::LIMIT,
            });
        }
        Ok(Self { month, used })
    }

    /// `month` の枠。同じ月ならそのまま、別の月（月をまたいだ）なら使った回数を 0 から数える。
    pub fn in_month(self, month: YearMonth) -> Self {
        if self.month == month {
            self
        } else {
            Self::fresh(month)
        }
    }

    pub const fn month(&self) -> YearMonth {
        self.month
    }

    /// この月に使った回数。
    pub const fn used(&self) -> u8 {
        self.used
    }

    /// 暦月あたりの上限（2）。
    pub const fn limit(&self) -> u8 {
        Self::LIMIT
    }

    /// この月の残り回数。
    pub const fn remaining(&self) -> u8 {
        Self::LIMIT - self.used
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ym(year: i32, month: u8) -> YearMonth {
        YearMonth::new(year, month).unwrap()
    }

    #[test]
    fn limit_is_2_per_calendar_month() {
        assert_eq!(LiveLessonQuota::LIMIT, 2);
        let quota = LiveLessonQuota::fresh(ym(2026, 10));
        assert_eq!((quota.used(), quota.limit(), quota.remaining()), (0, 2, 2));
    }

    #[test]
    fn restores_up_to_the_limit() {
        for used in 0..=2 {
            let quota = LiveLessonQuota::restore(ym(2026, 10), used).unwrap();
            assert_eq!((quota.used(), quota.remaining()), (used, 2 - used));
        }
        assert_eq!(
            LiveLessonQuota::restore(ym(2026, 10), 3),
            Err(DomainError::LiveLessonsUsedOverLimit { used: 3, limit: 2 })
        );
    }

    #[test]
    fn same_month_keeps_used() {
        let quota = LiveLessonQuota::restore(ym(2026, 10), 2).unwrap();
        assert_eq!(quota.in_month(ym(2026, 10)), quota);
    }

    #[test]
    fn crossing_into_the_next_month_resets_used_to_0() {
        let october = LiveLessonQuota::restore(ym(2026, 10), 2).unwrap();
        assert_eq!(october.remaining(), 0, "10 月は使い切っている");

        let november = october.in_month(ym(2026, 11));
        assert_eq!(november.month(), ym(2026, 11));
        assert_eq!(
            (november.used(), november.remaining()),
            (0, 2),
            "月をまたいだら 0 から"
        );
    }

    #[test]
    fn crossing_the_year_resets_used_to_0() {
        let december = LiveLessonQuota::restore(ym(2026, 12), 1).unwrap();
        let january = december.in_month(ym(2027, 1));
        assert_eq!(january, LiveLessonQuota::fresh(ym(2027, 1)));
    }
}
