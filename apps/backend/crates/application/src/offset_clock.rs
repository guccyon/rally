//! 実際の時刻より決まった秒数だけ進めた時計。触れる環境（staging）で、QA が「翌日」「暦月の境」をまたぐ流れを
//! 待たずに確かめるためのもの（factory-engine の ADR 0028 の scenario_time）。進める量は `server` が起動時に
//! 環境変数 `CLOCK_OFFSET_SECONDS` から決める。本番では渡さない（0 のまま）。

use std::time::{Duration, SystemTime};

use crate::clock::Clock;

/// OS の時計に `offset` を足した時刻を返す [`Clock`]。`offset` が 0 なら OS の時計と同じ。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OffsetClock {
    offset: Duration,
}

impl OffsetClock {
    pub fn new(offset: Duration) -> Self {
        Self { offset }
    }

    /// 進めている量。
    pub fn offset(&self) -> Duration {
        self.offset
    }
}

impl Clock for OffsetClock {
    fn now(&self) -> SystemTime {
        SystemTime::now() + self.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_offset_follows_the_system_clock() {
        let before = SystemTime::now();
        let now = OffsetClock::default().now();
        let after = SystemTime::now();
        assert!(before <= now && now <= after);
    }

    #[test]
    fn a_day_ahead_is_a_day_ahead_of_the_system_clock() {
        let day = Duration::from_secs(24 * 60 * 60);
        let before = SystemTime::now();
        let now = OffsetClock::new(day).now();
        let after = SystemTime::now();
        assert!(before + day <= now && now <= after + day);
        assert_eq!(OffsetClock::new(day).offset(), day);
    }
}
