//! 時刻がどの暦月か（[`YearMonth`]）。ビデオ通話の枠を区切る暦月は、ユースケースがここだけを通して決める。
//!
//! 暦月をどのタイムゾーンで区切るかは T11 で決める（docs/adr/0004 の 4）。それまでは UTC で区切る。
//! 変えるときはこの 1 か所を変える。

use std::time::SystemTime;

use domain::entitlement::YearMonth;
use time::OffsetDateTime;

/// `at` を含む暦月（UTC）。
pub fn of(at: SystemTime) -> YearMonth {
    let at = OffsetDateTime::from(at);
    YearMonth::new(at.year(), u8::from(at.month())).expect("time の月は 1〜12")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// 2026-11-01T00:00:00Z
    const NOV1: Duration = Duration::from_secs(1_793_491_200);

    #[test]
    fn splits_months_at_midnight_utc() {
        let nov1 = SystemTime::UNIX_EPOCH + NOV1;
        assert_eq!(of(nov1), YearMonth::new(2026, 11).unwrap());
        assert_eq!(
            of(nov1 - Duration::from_nanos(1)),
            YearMonth::new(2026, 10).unwrap()
        );
    }
}
