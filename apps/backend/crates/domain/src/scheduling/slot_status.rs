/// スロットの状態。外部での文字列表現は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotStatus {
    /// 公開中で、まだ予約されていない（生徒の検索に出る）
    Open,
    /// 予約で埋まっている（予約の作成は T08）
    Booked,
    /// コーチが閉じた。時間を占めないので、同じ時間に新しい枠を公開できる
    Closed,
}

impl SlotStatus {
    pub const ALL: [SlotStatus; 3] = [SlotStatus::Open, SlotStatus::Booked, SlotStatus::Closed];

    /// コーチの時間を占めるか（重なりの判定の対象か）。
    pub const fn occupies_time(self) -> bool {
        !matches!(self, SlotStatus::Closed)
    }
}
