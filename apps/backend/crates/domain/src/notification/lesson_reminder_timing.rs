use std::time::{Duration, SystemTime};

/// レッスンのリマインド（[`super::NotificationKind::LessonReminder`]）の時点。確定した同期レッスンの予約 1 件について、
/// 時点ごとに生徒とコーチへ 1 件ずつ作る。外部での文字列表現は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LessonReminderTiming {
    /// 前日（開始の 24 時間前から、開始 1 時間前の手前まで）
    DayBefore,
    /// 開始 1 時間前（開始 1 時間前から、開始の手前まで）
    HourBefore,
}

impl LessonReminderTiming {
    pub const ALL: [LessonReminderTiming; 2] = [
        LessonReminderTiming::DayBefore,
        LessonReminderTiming::HourBefore,
    ];

    /// リマインドの窓が開くのが開始のどれだけ前か。窓は開始のこれだけ前（含む）から、次に短い時点の窓が開くまで
    /// （1 時間前は開始まで。含まない）。
    pub const fn lead(self) -> Duration {
        match self {
            LessonReminderTiming::DayBefore => Duration::from_secs(24 * 60 * 60),
            LessonReminderTiming::HourBefore => Duration::from_secs(60 * 60),
        }
    }

    /// 開始が `starts_at` のレッスンについて、時刻 `now` に窓が開いている時点（なければ `None`）。開始の 24 時間前より前・
    /// 開始の時刻以後（始まった・終わったレッスン）は `None`。窓は重ならないので、時点は高々 1 つ。
    ///
    /// 定期の処理が窓の中で何度回っても、同じ時点を返す（重複を防ぐのは保存先。お知らせは (予約, 時点, 宛先) で 1 件）。
    /// 処理が止まっていて窓を過ぎたリマインドは作らない（過ぎた時点を返さない）。
    pub fn due_at(starts_at: SystemTime, now: SystemTime) -> Option<Self> {
        let until_start = starts_at.duration_since(now).ok()?;
        if until_start.is_zero() {
            return None;
        }
        if until_start <= LessonReminderTiming::HourBefore.lead() {
            Some(LessonReminderTiming::HourBefore)
        } else if until_start <= LessonReminderTiming::DayBefore.lead() {
            Some(LessonReminderTiming::DayBefore)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: Duration = Duration::from_secs(60 * 60);

    fn start() -> SystemTime {
        SystemTime::UNIX_EPOCH + 100 * HOUR
    }

    #[test]
    fn nothing_is_due_more_than_a_day_before() {
        assert_eq!(
            LessonReminderTiming::due_at(start(), start() - HOUR * 24 - Duration::from_secs(1)),
            None
        );
    }

    #[test]
    fn the_day_before_is_due_from_24_hours_to_just_over_an_hour_before() {
        assert_eq!(
            LessonReminderTiming::due_at(start(), start() - HOUR * 24),
            Some(LessonReminderTiming::DayBefore),
            "24 時間前ちょうどから"
        );
        assert_eq!(
            LessonReminderTiming::due_at(start(), start() - HOUR - Duration::from_secs(1)),
            Some(LessonReminderTiming::DayBefore)
        );
    }

    #[test]
    fn the_hour_before_is_due_from_an_hour_before_until_the_start() {
        assert_eq!(
            LessonReminderTiming::due_at(start(), start() - HOUR),
            Some(LessonReminderTiming::HourBefore),
            "1 時間前ちょうどから"
        );
        assert_eq!(
            LessonReminderTiming::due_at(start(), start() - Duration::from_secs(1)),
            Some(LessonReminderTiming::HourBefore)
        );
    }

    #[test]
    fn nothing_is_due_once_the_lesson_has_started() {
        assert_eq!(LessonReminderTiming::due_at(start(), start()), None);
        assert_eq!(LessonReminderTiming::due_at(start(), start() + HOUR), None);
    }
}
