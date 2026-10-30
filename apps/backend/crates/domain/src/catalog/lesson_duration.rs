use crate::DomainError;

/// 同期レッスンの 1 回の時間（分）。[`LessonDuration::MIN_MINUTES`]〜[`LessonDuration::MAX_MINUTES`] の、
/// [`LessonDuration::STEP_MINUTES`] 分刻みの整数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LessonDuration(u8);

impl LessonDuration {
    /// 最短（分。admin-web の契約の `minimum`）
    pub const MIN_MINUTES: u8 = 15;
    /// 最長（分。admin-web の契約の `maximum`）
    pub const MAX_MINUTES: u8 = 120;
    /// 刻み（分。admin-web の契約の `multipleOf`）
    pub const STEP_MINUTES: u8 = 15;

    /// `minutes` が 15〜120 の 15 分刻みでなければ [`DomainError::LessonDurationOutOfRange`]。
    pub fn new(minutes: i64) -> Result<Self, DomainError> {
        u8::try_from(minutes)
            .ok()
            .filter(|minutes| {
                (Self::MIN_MINUTES..=Self::MAX_MINUTES).contains(minutes)
                    && minutes % Self::STEP_MINUTES == 0
            })
            .map(Self)
            .ok_or(DomainError::LessonDurationOutOfRange {
                min: Self::MIN_MINUTES,
                max: Self::MAX_MINUTES,
                step: Self::STEP_MINUTES,
            })
    }

    pub const fn minutes(self) -> u8 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_every_quarter_hour_from_15_to_120() {
        let accepted: Vec<u8> = (0..=200)
            .filter_map(|minutes| LessonDuration::new(minutes).ok())
            .map(LessonDuration::minutes)
            .collect();
        assert_eq!(accepted, [15, 30, 45, 60, 75, 90, 105, 120]);
    }

    #[test]
    fn rejects_other_values() {
        for minutes in [0, 14, 16, 50, 135, -15, 255, 270, i64::MAX, i64::MIN] {
            assert_eq!(
                LessonDuration::new(minutes),
                Err(DomainError::LessonDurationOutOfRange {
                    min: 15,
                    max: 120,
                    step: 15
                }),
                "{minutes}"
            );
        }
    }
}
