//! KPI の値が出せない理由。

/// KPI の 1 つの期間の値が出せない理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KpiUnavailableReason {
    /// 期間に母集団が 1 行もない（`avg`・`median` では null でない値が 1 つもない）
    NoPopulation,
    /// 群ごとに数える指標（`variance`）で、件数の下限（`having count >= min_count`）を満たす群がない
    BelowGroupMinimum { min_count: u32 },
    /// 期間の母集団の観測がまだ終わっていない。母集団は集計した時刻から `observe_days` 日より前の行だけを数えるが、
    /// それより新しい行しかまだない
    NotObservableYet { observe_days: u32 },
    /// 集計に失敗した（イベントを読めない・式を評価できない）
    Unavailable,
}
