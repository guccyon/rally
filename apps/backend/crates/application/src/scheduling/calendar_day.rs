//! 時刻がどの日付か（`time::Date`）と、日付の始まりと終わりの時刻。本部が見る予約の一覧（[`super::ListBookings`]）と
//! キャンセルされた予約の一覧（[`super::ListCancelledBookings`]）と、コーチの自分の枠の一覧（[`super::ListCoachSlots`]）の
//! 日付は、ユースケースがここだけを通して決める。
//!
//! 日付は暦月と同じく日本時間（UTC+9。夏時間はない）で区切る（[`crate::entitlement::calendar_month`] のタイムゾーン）。

use std::time::SystemTime;

use domain::scheduling::{BookingPeriod, SchedulingError};
use time::{Date, Duration, OffsetDateTime, Time};

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

/// 開始の日付で絞る一覧（admin-web の `listBookings`・`listMyCoachSlots`）の期間。日付（日本時間）の `from` から `to`
/// まで（両端を含む）で、`from` を省けば今日（`now` の日付）、`to` を省けば `from` から [`BookingPeriod::DEFAULT_DAYS`]
/// 日目。決めた `from`・`to` と、その期間を返す。期間の規則（`to` が `from` より前でない・[`BookingPeriod::MAX_DAYS`]
/// 日まで）に反すれば domain の [`BookingPeriod::new`] の失敗。
pub fn forward_period(
    from: Option<Date>,
    to: Option<Date>,
    now: SystemTime,
) -> Result<(Date, Date, BookingPeriod), SchedulingError> {
    let from = from.unwrap_or_else(|| of(now));
    let to = to.unwrap_or_else(|| {
        from.saturating_add(Duration::days(i64::from(BookingPeriod::DEFAULT_DAYS) - 1))
    });
    let period = BookingPeriod::new(start(from), end(to))?;
    Ok((from, to, period))
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

    #[test]
    fn forward_period_defaults_to_today_and_the_default_days_from_from() {
        let nov1 = SystemTime::UNIX_EPOCH + NOV1_JST;
        let today = date(Month::November, 1);
        let (from, to, period) = forward_period(None, None, nov1).unwrap();
        assert_eq!((from, to), (today, date(Month::November, 7)));
        assert_eq!(
            (period.starts_at(), period.ends_at()),
            (start(today), end(date(Month::November, 7)))
        );
        let (from, to, _) = forward_period(Some(date(Month::November, 10)), None, nov1).unwrap();
        assert_eq!(
            (from, to),
            (date(Month::November, 10), date(Month::November, 16))
        );
        let (from, to, _) = forward_period(None, Some(date(Month::November, 3)), nov1).unwrap();
        assert_eq!((from, to), (today, date(Month::November, 3)));
        let (from, to, _) = forward_period(
            Some(date(Month::November, 1)),
            Some(date(Month::December, 1)),
            nov1,
        )
        .unwrap();
        assert_eq!((from, to), (today, date(Month::December, 1)));
    }

    #[test]
    fn forward_period_follows_the_period_rules() {
        let nov1 = SystemTime::UNIX_EPOCH + NOV1_JST;
        assert_eq!(
            forward_period(None, Some(date(Month::October, 31)), nov1),
            Err(SchedulingError::BookingPeriodEndsBeforeItStarts),
            "to だけで今日より前なら from（今日）より前"
        );
        assert_eq!(
            forward_period(
                Some(date(Month::November, 1)),
                Some(date(Month::December, 2)),
                nov1
            ),
            Err(SchedulingError::BookingPeriodTooLong { max_days: 31 })
        );
    }
}
