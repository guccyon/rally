//! KPI の式と、期間を当てる属性。

use super::KpiFormula;

/// kpi.yaml の `formula` と `window_on`。
#[derive(Debug, Clone, PartialEq)]
pub struct KpiWindowedFormula {
    pub formula: KpiFormula,
    /// `window_on` の属性（`<別名>.<属性>` の属性。別名は母集団の別名と同じ）
    pub window_on: String,
}
