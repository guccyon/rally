//! 時刻がどの日付か（`time::Date`）と、日付の始まりと終わりの時刻。本部が見る予約の一覧（[`super::ListBookings`]）・
//! キャンセルされた予約の一覧（[`super::ListCancelledBookings`]）・予約枠の需給（[`super::GetSlotSupply`]）・コーチの自分の
//! 予約の一覧（[`super::ListCoachBookings`]）・コーチの自分の枠の一覧（[`super::ListCoachSlots`]）・本部が送ったお知らせの
//! 一覧（[`crate::notification::ListAnnouncements`]）の日付は、ユースケースがここだけを通して決める。省いた日付の決め方が
//! `listBookings` と同じ照会（`listBookings`・`getSlotSupply`・`listMyCoachBookings`・`listMyCoachSlots`）は [`period`] を
//! 通し、過去を引く照会（`listCancelledBookings`・`listAnnouncements`）は [`period_ending`] を通す。
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

/// `at` の日本時間の日時（画面やお知らせに日時を書くとき。曜日・時・分を読む）。
pub fn local(at: SystemTime) -> OffsetDateTime {
    OffsetDateTime::from(at).to_offset(OFFSET)
}

/// 日付 `date` の始まり（日本時間の 0 時）。
pub fn start(date: Date) -> SystemTime {
    date.with_time(Time::MIDNIGHT).assume_offset(OFFSET).into()
}

/// 日付 `date` の終わり（翌日の日本時間の 0 時。含まない）。夏時間がないので始まりの 24 時間後。
pub fn end(date: Date) -> SystemTime {
    start(date) + BookingPeriod::DAY
}

/// 日付（日本時間）の `from` から `to` まで（両端を含む）の期間（admin-web の `getCoachLeavePreview`・`startCoachLeave`。
/// どちらの日付も省けない）。期間の規則は [`period`] と同じ domain の [`BookingPeriod::new`] で、違反はその [`SchedulingError`]。
pub fn days(from: Date, to: Date) -> Result<BookingPeriod, SchedulingError> {
    BookingPeriod::new(start(from), end(to))
}

/// 日付で決めた期間 `period`（[`days`]）の始まりの日と終わりの日（両端を含む）。
pub fn dates_of(period: BookingPeriod) -> (Date, Date) {
    (
        of(period.starts_at()),
        of(period.ends_at() - std::time::Duration::from_nanos(1)),
    )
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

/// 日付（日本時間）の `from` から `to` まで（両端を含む）の期間と、実際に使った `from`・`to`（admin-web の
/// `listCancelledBookings`・`listAnnouncements` の規則。過去を引くので終わりの日付から数える）。`from` だけなら `from` から
/// [`BookingPeriod::DEFAULT_DAYS`] 日目まで、`to` だけなら `to` までの [`BookingPeriod::DEFAULT_DAYS`] 日、両方省けば
/// `today`（両方省いたときだけ呼ぶ）までの [`BookingPeriod::DEFAULT_DAYS`] 日。期間の規則は [`period`] と同じ domain の
/// [`BookingPeriod::new`] で、違反はその [`SchedulingError`]。
pub fn period_ending(
    from: Option<Date>,
    to: Option<Date>,
    today: impl FnOnce() -> Date,
) -> Result<(Date, Date, BookingPeriod), SchedulingError> {
    let span = Duration::days(i64::from(BookingPeriod::DEFAULT_DAYS) - 1);
    let (from, to) = match (from, to) {
        (Some(from), Some(to)) => (from, to),
        (Some(from), None) => (from, from.saturating_add(span)),
        (None, to) => {
            let to = to.unwrap_or_else(today);
            (to.saturating_sub(span), to)
        }
    };
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
    fn days_round_trip_through_the_period() {
        let period = days(date(Month::November, 10), date(Month::November, 16)).unwrap();
        assert_eq!(period.starts_at(), start(date(Month::November, 10)));
        assert_eq!(period.ends_at(), end(date(Month::November, 16)));
        assert_eq!(
            dates_of(period),
            (date(Month::November, 10), date(Month::November, 16))
        );
        let one = days(date(Month::November, 10), date(Month::November, 10)).unwrap();
        assert_eq!(
            dates_of(one),
            (date(Month::November, 10), date(Month::November, 10))
        );
        assert_eq!(
            days(date(Month::November, 10), date(Month::November, 9)),
            Err(SchedulingError::BookingPeriodEndsBeforeItStarts)
        );
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

    #[test]
    fn period_ending_counts_seven_days_back_to_today_or_to() {
        let today = || date(Month::November, 1);
        let (from, to, period) = period_ending(None, None, today).unwrap();
        assert_eq!(
            (from, to),
            (date(Month::October, 26), date(Month::November, 1)),
            "両方省けば今日までの 7 日"
        );
        assert_eq!(period.starts_at(), start(from));
        assert_eq!(period.ends_at(), end(to));
        assert_eq!(
            period_ending(None, Some(date(Month::November, 3)), today)
                .unwrap()
                .0,
            date(Month::October, 28),
            "to だけなら to の 6 日前から"
        );
        assert_eq!(
            period_ending(Some(date(Month::November, 1)), None, today)
                .unwrap()
                .1,
            date(Month::November, 7),
            "from だけなら from から 7 日目まで"
        );
        let given = period_ending(
            Some(date(Month::November, 1)),
            Some(date(Month::November, 1)),
            || unreachable!("日付があれば今日は読まない"),
        )
        .unwrap();
        assert_eq!(
            (given.0, given.1),
            (date(Month::November, 1), date(Month::November, 1))
        );
    }

    #[test]
    fn period_ending_follows_the_same_domain_rule_as_period() {
        let today = || date(Month::November, 1);
        assert_eq!(
            period_ending(
                Some(date(Month::November, 2)),
                Some(date(Month::November, 1)),
                today
            ),
            Err(SchedulingError::BookingPeriodEndsBeforeItStarts)
        );
        assert_eq!(
            period_ending(
                Some(date(Month::November, 1)),
                Some(date(Month::December, 2)),
                today
            ),
            Err(SchedulingError::BookingPeriodTooLong { max_days: 31 })
        );
    }
}
