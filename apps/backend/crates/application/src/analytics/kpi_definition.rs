//! kpi.yaml の KPI 1 つの定義。

use std::time::Duration;

use super::{KpiDirection, KpiWindowedFormula};

/// kpi.yaml の `kpis` の 1 項目。式が読めなかった項目も、名前・向き・期間の長さが読めれば残し（`formula` が理由の文）、
/// その指標だけを出せなくする。
#[derive(Debug, Clone, PartialEq)]
pub struct KpiDefinition {
    pub id: String,
    /// `ja`
    pub name: String,
    pub north_star: bool,
    pub direction: KpiDirection,
    /// `window` の長さ
    pub window: Duration,
    /// 式（`formula`）と、期間を当てる属性（`window_on`）。読めなければ理由の文
    pub formula: Result<KpiWindowedFormula, String>,
}
