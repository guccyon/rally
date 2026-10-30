use crate::DomainError;

/// 同期レッスン 1 回の時間（分）。[`LessonDuration::MIN_MINUTES`]〜[`LessonDuration::MAX_MINUTES`] の
/// [`LessonDuration::STEP_MINUTES`] 分刻み。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LessonDuration(u8);

impl LessonDuration {
    pub const MIN_MINUTES: u8 = 15;
    pub const MAX_MINUTES: u8 = 120;
    pub const STEP_MINUTES: u8 = 15;

    pub fn from_minutes(minutes: i64) -> Result<Self, DomainError> {
        let out_of_range = || DomainError::LessonDurationOutOfRange {
            min: Self::MIN_MINUTES,
            max: Self::MAX_MINUTES,
            step: Self::STEP_MINUTES,
        };
        let minutes = u8::try_from(minutes).map_err(|_| out_of_range())?;
        if !(Self::MIN_MINUTES..=Self::MAX_MINUTES).contains(&minutes)
            || minutes % Self::STEP_MINUTES != 0
        {
            return Err(out_of_range());
        }
        Ok(Self(minutes))
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
        let accepted: Vec<u8> = (-1..=200)
            .filter_map(|m| LessonDuration::from_minutes(m).ok())
            .map(LessonDuration::minutes)
            .collect();
        assert_eq!(accepted, [15, 30, 45, 60, 75, 90, 105, 120]);
    }

    #[test]
    fn other_minutes_are_a_domain_error() {
        for minutes in [0, 14, 20, 125, 135, 255, 256, -15, i64::MAX] {
            assert_eq!(
                LessonDuration::from_minutes(minutes),
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
