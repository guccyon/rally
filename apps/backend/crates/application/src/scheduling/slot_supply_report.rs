use std::time::SystemTime;

use domain::scheduling::SlotSupply;
use time::Date;

/// スポーツごとの予約枠の需給（[`super::GetSlotSupply`] の結果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotSupplyReport {
    /// 数えた期間の始まり（含む。日付を指定しなければ集計した時刻の [`SlotSupply::WINDOW`] 前、指定すれば `from` の
    /// 日本時間 0 時）
    pub window_start: SystemTime,
    /// 数えた期間の終わり（含まない。日付を指定しなければ集計した時刻、指定すれば `to` の翌日の日本時間 0 時）
    pub window_end: SystemTime,
    /// 日付で指定したときに実際に使った `from` と `to`（日本時間。両端を含む。省いた片方を決めた日付も含む）。
    /// 日付を両方省いたときは `None`
    pub dates: Option<(Date, Date)>,
    /// スポーツ 6 種類すべて（[`domain::catalog::Sport::ALL`] の順）
    pub sports: Vec<SlotSupply>,
}
