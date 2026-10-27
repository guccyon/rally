use std::time::SystemTime;

use domain::scheduling::SlotSupply;

/// スポーツごとの予約枠の需給（[`super::GetSlotSupply`] の結果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotSupplyReport {
    /// 数えた期間の始まり（含む。集計した時刻の [`SlotSupply::WINDOW`] 前）
    pub window_start: SystemTime,
    /// 数えた期間の終わり（含まない。集計した時刻）
    pub window_end: SystemTime,
    /// スポーツ 5 種類すべて（[`domain::identity::Sport::ALL`] の順）
    pub sports: Vec<SlotSupply>,
}
