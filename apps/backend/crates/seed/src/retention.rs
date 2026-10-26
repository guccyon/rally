/// 生徒の継続の分布の区分（Blueprint の synthetic_seed: 初回レッスン後 30 日以内の離脱が最多）。
///
/// 離脱は「その時刻より後に生徒の出来事がない」こと（月額契約の解約は Gen1 の行動イベントにないので扱わない）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Retention {
    /// 初回レッスンを受けずに離脱する（体験予約のキャンセルだけの生徒を含む）
    NoLesson,
    /// 初回レッスン後 30 日以内に離脱する
    Within30Days,
    /// 初回レッスン後 31〜90 日に離脱する
    Within90Days,
    /// 初回レッスン後 91〜180 日に離脱する
    Within180Days,
    /// 初回レッスン後 181 日以降に離脱する
    After180Days,
    /// 離脱せず、直近 30 日にも活動している
    Active,
}

impl Retention {
    pub const ALL: [Retention; 6] = [
        Retention::NoLesson,
        Retention::Within30Days,
        Retention::Within90Days,
        Retention::Within180Days,
        Retention::After180Days,
        Retention::Active,
    ];

    /// 初回レッスンから離脱までの日数の範囲（`NoLesson` と `Active` は `None`）。
    pub const fn days_after_first_lesson(self) -> Option<(u64, u64)> {
        match self {
            Retention::Within30Days => Some((0, 30)),
            Retention::Within90Days => Some((31, 90)),
            Retention::Within180Days => Some((91, 180)),
            Retention::After180Days => Some((181, 365)),
            Retention::NoLesson | Retention::Active => None,
        }
    }
}
