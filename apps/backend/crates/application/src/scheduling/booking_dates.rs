use domain::scheduling::{BookingPeriod, SchedulingError};
use time::{Date, Duration};

use super::calendar_day;

/// 予約されたスロットの開始の日付（日本時間）で予約を絞る期間。本部が見る予約の一覧（[`super::ListBookings`]。admin-web の
/// `listBookings`）とコーチの自分の予約の一覧（[`super::ListCoachBookings`]。`listMyCoachBookings`）は、日付の `from`・`to`
/// をここだけを通して期間にする。
///
/// `from` を省けば今日、`to` を省けば `from` から [`BookingPeriod::DEFAULT_DAYS`] 日目（`from` を含めて数える）。
/// 期間の規則（`to` が `from` より前でない・[`BookingPeriod::MAX_DAYS`] 日まで）は domain の [`BookingPeriod`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BookingDates {
    /// 期間の始まりの日付（日本時間。含む）。省いたときは今日
    pub from: Date,
    /// 期間の終わりの日付（日本時間。含む）。省いたときは `from` から 7 日目
    pub to: Date,
    /// `from` の日本時間 0 時（含む）から `to` の翌日の日本時間 0 時（含まない）まで
    pub period: BookingPeriod,
}

impl BookingDates {
    /// 省いた日付を `today`（サーバーの現在時刻の日本時間の日付）から決め、期間にする。期間が規則を満たさなければ
    /// [`BookingPeriod::new`] の失敗。
    pub fn resolve(
        from: Option<Date>,
        to: Option<Date>,
        today: Date,
    ) -> Result<Self, SchedulingError> {
        let from = from.unwrap_or(today);
        let to = to.unwrap_or_else(|| {
            from.saturating_add(Duration::days(i64::from(BookingPeriod::DEFAULT_DAYS) - 1))
        });
        let period = BookingPeriod::new(calendar_day::start(from), calendar_day::end(to))?;
        Ok(Self { from, to, period })
    }
}

#[cfg(test)]
mod tests {
    use time::Month;

    use super::*;

    fn date(day: u8) -> Date {
        Date::from_calendar_date(2026, Month::November, day).unwrap()
    }

    #[test]
    fn omitted_dates_default_to_seven_days_from_today() {
        let dates = BookingDates::resolve(None, None, date(2)).unwrap();
        assert_eq!((dates.from, dates.to), (date(2), date(8)));
        assert_eq!(dates.period.starts_at(), calendar_day::start(date(2)));
        assert_eq!(dates.period.ends_at(), calendar_day::end(date(8)));
    }

    #[test]
    fn omitted_to_is_the_seventh_day_from_from() {
        let dates = BookingDates::resolve(Some(date(10)), None, date(2)).unwrap();
        assert_eq!((dates.from, dates.to), (date(10), date(16)));
    }

    #[test]
    fn omitted_from_is_today() {
        let dates = BookingDates::resolve(None, Some(date(3)), date(2)).unwrap();
        assert_eq!((dates.from, dates.to), (date(2), date(3)));
    }

    #[test]
    fn one_day_and_thirty_one_days_are_accepted() {
        assert!(BookingDates::resolve(Some(date(5)), Some(date(5)), date(2)).is_ok());
        let last = Date::from_calendar_date(2026, Month::December, 5).unwrap();
        assert!(BookingDates::resolve(Some(date(5)), Some(last), date(2)).is_ok());
    }

    #[test]
    fn rejects_to_before_from_and_periods_longer_than_max_days() {
        assert_eq!(
            BookingDates::resolve(Some(date(5)), Some(date(4)), date(2)),
            Err(SchedulingError::BookingPeriodEndsBeforeItStarts)
        );
        assert_eq!(
            BookingDates::resolve(None, Some(date(1)), date(2)),
            Err(SchedulingError::BookingPeriodEndsBeforeItStarts),
            "to だけを付けて今日より前なら、from（今日）より前"
        );
        let too_far = Date::from_calendar_date(2026, Month::December, 6).unwrap();
        assert_eq!(
            BookingDates::resolve(Some(date(5)), Some(too_far), date(2)),
            Err(SchedulingError::BookingPeriodTooLong { max_days: 31 })
        );
    }
}
