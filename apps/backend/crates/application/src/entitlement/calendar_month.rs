//! 時刻がどの暦月か（[`YearMonth`]）と、暦月の始まりの時刻。ビデオ通話の枠と、コーチの月ごとのレッスンの実績
//! （`lesson::GetMyLessonStats`）を区切る暦月は、ユースケースがここだけを通して決める。
//!
//! 暦月は日本時間（UTC+9。夏時間はない）で区切る（docs/adr/0005-live-lesson-quota-consumption.md）。
//! 変えるときはこの 1 か所を変える。

use std::time::SystemTime;

use domain::entitlement::YearMonth;
use time::{Date, Month, OffsetDateTime, Time, UtcOffset};

/// 暦月を区切るタイムゾーン（日本時間）。
const OFFSET: UtcOffset = match UtcOffset::from_hms(9, 0, 0) {
    Ok(offset) => offset,
    Err(_) => panic!("UTC+9 は有効なオフセット"),
};

/// `at` を含む暦月（日本時間）。
pub fn of(at: SystemTime) -> YearMonth {
    let at = OffsetDateTime::from(at).to_offset(OFFSET);
    YearMonth::new(at.year(), u8::from(at.month())).expect("time の月は 1〜12")
}

/// 暦月 `month` の始まり（日本時間の 1 日 0 時）。
pub fn start(month: YearMonth) -> SystemTime {
    let month_of_year = Month::try_from(month.month()).expect("YearMonth の月は 1〜12");
    let first =
        Date::from_calendar_date(month.year(), month_of_year, 1).expect("どの暦月にも 1 日はある");
    first.with_time(Time::MIDNIGHT).assume_offset(OFFSET).into()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// 2026-11-01T00:00:00+09:00（2026-10-31T15:00:00Z）
    const NOV1_JST: Duration = Duration::from_secs(1_793_458_800);

    #[test]
    fn splits_months_at_midnight_in_japan() {
        let nov1 = SystemTime::UNIX_EPOCH + NOV1_JST;
        assert_eq!(of(nov1), YearMonth::new(2026, 11).unwrap());
        assert_eq!(
            of(nov1 - Duration::from_nanos(1)),
            YearMonth::new(2026, 10).unwrap()
        );
    }

    #[test]
    fn a_month_starts_at_midnight_on_the_first_in_japan() {
        let nov1 = SystemTime::UNIX_EPOCH + NOV1_JST;
        assert_eq!(start(YearMonth::new(2026, 11).unwrap()), nov1);
        assert_eq!(
            of(start(YearMonth::new(2027, 1).unwrap())),
            YearMonth::new(2027, 1).unwrap()
        );
    }

    #[test]
    fn midnight_utc_on_the_first_is_still_the_new_month_in_japan() {
        // 2026-12-01T00:00:00Z は日本時間の 12 月 1 日 9 時
        let dec1_utc = SystemTime::UNIX_EPOCH + Duration::from_secs(1_796_083_200);
        assert_eq!(of(dec1_utc), YearMonth::new(2026, 12).unwrap());
        // 2026-11-30T15:00:00Z は日本時間の 12 月 1 日 0 時
        assert_eq!(
            of(dec1_utc - Duration::from_secs(9 * 3600)),
            YearMonth::new(2026, 12).unwrap()
        );
        assert_eq!(
            of(dec1_utc - Duration::from_secs(9 * 3600 + 1)),
            YearMonth::new(2026, 11).unwrap()
        );
    }
}
