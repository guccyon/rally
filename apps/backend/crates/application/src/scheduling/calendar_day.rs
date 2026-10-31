//! 時刻がどの日付か（`time::Date`）と、日付の始まりと終わりの時刻。本部が見る予約の一覧（[`super::ListBookings`]）の
//! 日付は、ユースケースがここだけを通して決める。
//!
//! 日付は暦月と同じく日本時間（UTC+9。夏時間はない）で区切る（[`crate::entitlement::calendar_month`] のタイムゾーン）。

use std::time::SystemTime;

use domain::scheduling::BookingPeriod;
use time::{Date, OffsetDateTime, Time};

use crate::entitlement::calendar_month::OFFSET;

/// `at` を含む日付（日本時間）。
pub fn of(at: SystemTime) -> Date {
    OffsetDateTime::from(at).to_offset(OFFSET).date()
}

/// 日付 `date` の始まり（日本時間の 0 時）。
pub fn start(date: Date) -> SystemTime {
    date.with_time(Time::MIDNIGHT).assume_offset(OFFSET).into()
}

/// 日付 `date` の終わり（翌日の日本時間の 0 時。含まない）。夏時間がないので始まりの 24 時間後。
pub fn end(date: Date) -> SystemTime {
    start(date) + BookingPeriod::DAY
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use time::Month;

    use super::*;

    /// 2026-11-01T00:00:00+09:00（2026-10-31T15:00:00Z）
    const NOV1_JST: Duration = Duration::from_secs(1_793_458_800);

    fn date(month: Month, day: u8) -> Date {
        Date::from_calendar_date(2026, month, day).unwrap()
    }

    #[test]
    fn splits_days_at_midnight_in_japan() {
        let nov1 = SystemTime::UNIX_EPOCH + NOV1_JST;
        assert_eq!(of(nov1), date(Month::November, 1));
        assert_eq!(of(nov1 - Duration::from_nanos(1)), date(Month::October, 31));
    }

    #[test]
    fn a_day_starts_and_ends_at_midnight_in_japan() {
        let nov1 = SystemTime::UNIX_EPOCH + NOV1_JST;
        assert_eq!(start(date(Month::November, 1)), nov1);
        assert_eq!(end(date(Month::October, 31)), nov1);
        assert_eq!(
            end(date(Month::November, 1)),
            start(date(Month::November, 2))
        );
    }
}
