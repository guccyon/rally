use std::time::{Duration, SystemTime};

use super::SchedulingError;

/// 本部が見る予約の一覧（admin-web の `listBookings`）の期間。開始 `starts_at`（含む）から終了 `ends_at`（含まない）まで。
///
/// 日付（日本時間）の `from` の 0 時から `to` の翌日 0 時までにあたる。日付から時刻への変換（タイムゾーン）は
/// application が決め、ここは期間の長さの規則だけを持つ。期間は 1 日以上 [`Self::MAX_DAYS`] 日まで。日付を省いたときの
/// 長さは [`Self::DEFAULT_DAYS`] 日。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BookingPeriod {
    starts_at: SystemTime,
    ends_at: SystemTime,
}

impl BookingPeriod {
    /// 1 日の長さ（日本時間に夏時間はないので、どの日も 24 時間）
    pub const DAY: Duration = Duration::from_secs(24 * 3600);
    /// 終わりの日付を省いたときの期間の日数（始まりの日を含めて 7 日）
    pub const DEFAULT_DAYS: u16 = 7;
    /// 期間の日数の上限（始まりの日と終わりの日を含めて 31 日）
    pub const MAX_DAYS: u16 = 31;

    /// 終了が開始より後でなければ [`SchedulingError::BookingPeriodEndsBeforeItStarts`]、[`Self::MAX_DAYS`] 日より
    /// 長ければ [`SchedulingError::BookingPeriodTooLong`]。
    pub fn new(starts_at: SystemTime, ends_at: SystemTime) -> Result<Self, SchedulingError> {
        let length = match ends_at.duration_since(starts_at) {
            Ok(length) if !length.is_zero() => length,
            _ => return Err(SchedulingError::BookingPeriodEndsBeforeItStarts),
        };
        if length > Self::DAY * u32::from(Self::MAX_DAYS) {
            return Err(SchedulingError::BookingPeriodTooLong {
                max_days: Self::MAX_DAYS,
            });
        }
        Ok(Self { starts_at, ends_at })
    }

    pub const fn starts_at(&self) -> SystemTime {
        self.starts_at
    }

    pub const fn ends_at(&self) -> SystemTime {
        self.ends_at
    }

    /// `at` が期間に入るか（開始を含み、終了を含まない）。
    pub fn includes(&self, at: SystemTime) -> bool {
        self.starts_at <= at && at < self.ends_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(n: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + BookingPeriod::DAY * n as u32
    }

    #[test]
    fn accepts_one_to_max_days() {
        let one = BookingPeriod::new(day(10), day(11)).unwrap();
        assert_eq!((one.starts_at(), one.ends_at()), (day(10), day(11)));
        assert!(BookingPeriod::new(day(10), day(10 + u64::from(BookingPeriod::MAX_DAYS))).is_ok());
    }

    #[test]
    fn rejects_periods_that_end_before_or_when_they_start() {
        for ends_at in [day(10), day(9)] {
            assert_eq!(
                BookingPeriod::new(day(10), ends_at),
                Err(SchedulingError::BookingPeriodEndsBeforeItStarts)
            );
        }
    }

    #[test]
    fn rejects_periods_longer_than_max_days() {
        let ends_at = day(10 + u64::from(BookingPeriod::MAX_DAYS));
        assert_eq!(
            BookingPeriod::new(day(10), ends_at + Duration::from_secs(1)),
            Err(SchedulingError::BookingPeriodTooLong { max_days: 31 })
        );
    }

    #[test]
    fn includes_the_start_but_not_the_end() {
        let period = BookingPeriod::new(day(10), day(17)).unwrap();
        assert!(period.includes(day(10)));
        assert!(period.includes(day(17) - Duration::from_nanos(1)));
        assert!(!period.includes(day(17)));
        assert!(!period.includes(day(10) - Duration::from_nanos(1)));
    }
}
