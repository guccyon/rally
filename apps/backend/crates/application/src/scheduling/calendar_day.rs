//! 時刻がどの日付か（`time::Date`）と、日付の始まりと終わりの時刻。本部が見る予約の一覧（[`super::ListBookings`]）と
//! キャンセルされた予約の一覧（[`super::ListCancelledBookings`]）と予約枠の需給（[`super::GetSlotSupply`]）と、コーチの自分の
//! 枠の一覧（[`super::ListCoachSlots`]）の日付は、ユースケースがここだけを通して決める。省いた日付の決め方が `listBookings` と
//! 同じ照会（`listBookings`・`getSlotSupply`・`listMyCoachSlots`）は [`period`] を通す。
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

/// 日付（日本時間）の `from` から `to` まで（両端を含む）の期間と、実際に使った `from`・`to`（admin-web の
/// `listBookings` の規則）。`from` を省けば `now` の日付（今日）、`to` を省けば `from` から
/// [`BookingPeriod::DEFAULT_DAYS`] 日目。期間の規則（`to` が `from` より前でない・[`BookingPeriod::MAX_DAYS`] 日まで）は
/// domain の [`BookingPeriod::new`] で、違反はその [`SchedulingError`]。
pub fn period(
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
    fn period_defaults_to_seven_days_from_today_in_japan() {
        // 日本時間の 11 月 1 日 0 時の 1 ナノ秒前は 10 月 31 日
        let now = SystemTime::UNIX_EPOCH + NOV1_JST - Duration::from_nanos(1);
        let (from, to, period) = super::period(None, None, now).unwrap();
        assert_eq!(
            (from, to),
            (date(Month::October, 31), date(Month::November, 6))
        );
        assert_eq!(period.starts_at(), start(from));
        assert_eq!(period.ends_at(), end(to));

        let (from, to, _) = super::period(Some(date(Month::November, 10)), None, now).unwrap();
        assert_eq!(
            (from, to),
            (date(Month::November, 10), date(Month::November, 16))
        );
        let (from, to, _) = super::period(None, Some(date(Month::November, 2)), now).unwrap();
        assert_eq!(
            (from, to),
            (date(Month::October, 31), date(Month::November, 2))
        );
    }

    #[test]
    fn period_includes_both_dates_and_follows_the_domain_rule() {
        let now = SystemTime::UNIX_EPOCH + NOV1_JST;
        let one_day = super::period(
            Some(date(Month::November, 1)),
            Some(date(Month::November, 1)),
            now,
        )
        .unwrap()
        .2;
        assert_eq!(one_day.starts_at(), now);
        assert_eq!(one_day.ends_at(), now + BookingPeriod::DAY);
        // 31 日（11 月 1 日から 12 月 1 日）までは通り、32 日は通らない
        assert!(
            super::period(
                Some(date(Month::November, 1)),
                Some(date(Month::December, 1)),
                now
            )
            .is_ok()
        );
        assert_eq!(
            super::period(
                Some(date(Month::November, 1)),
                Some(date(Month::December, 2)),
                now
            ),
            Err(SchedulingError::BookingPeriodTooLong { max_days: 31 })
        );
        assert_eq!(
            super::period(
                Some(date(Month::November, 2)),
                Some(date(Month::November, 1)),
                now
            ),
            Err(SchedulingError::BookingPeriodEndsBeforeItStarts)
        );
        // `to` だけで今日より前は `from`（今日）より前
        assert_eq!(
            super::period(None, Some(date(Month::October, 31)), now),
            Err(SchedulingError::BookingPeriodEndsBeforeItStarts)
        );
    }
}
