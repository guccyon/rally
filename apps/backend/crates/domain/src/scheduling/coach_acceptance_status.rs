/// コーチの受付の状態（glossary の coach_acceptance の states）。外部での文字列表現は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoachAcceptanceStatus {
    /// 新しい予約を受け付ける（既定）
    Accepting,
    /// 本部が受付を止めている。枠は生徒の検索とコーチを探す一覧に出ず、新しい予約を受けない（入っている予約はそのまま）
    Suspended,
}

impl CoachAcceptanceStatus {
    pub const ALL: [CoachAcceptanceStatus; 2] = [
        CoachAcceptanceStatus::Accepting,
        CoachAcceptanceStatus::Suspended,
    ];
}
