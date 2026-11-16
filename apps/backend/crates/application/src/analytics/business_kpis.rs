//! 本部が見る事業の KPI（kpi.yaml の順）。

use std::time::SystemTime;

use super::BusinessKpi;

/// 事業の KPI（[`super::GetBusinessKpis`] の結果）。`kpis` は kpi.yaml の `kpis` の順で、値が出せない指標も省かない。
#[derive(Debug, Clone, PartialEq)]
pub struct BusinessKpis {
    /// 集計した時刻（式の `now`）
    pub computed_at: SystemTime,
    pub kpis: Vec<BusinessKpi>,
}
