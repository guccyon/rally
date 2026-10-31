use crate::catalog::Sport;

/// 生徒の値オブジェクト。レッスンを受けるスポーツを 1 つ持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Student {
    sport: Sport,
}

impl Student {
    pub const fn new(sport: Sport) -> Self {
        Self { sport }
    }

    pub const fn sport(&self) -> Sport {
        self.sport
    }
}
