//! 事業の KPI 1 つの今の期間と前の期間。

use super::{BusinessKpiValue, KpiDirection};

/// 事業の KPI 1 つ。`id`・`name`・`north_star`・`direction` は kpi.yaml の同じ項目のまま。
#[derive(Debug, Clone, PartialEq)]
pub struct BusinessKpi {
    pub id: String,
    /// kpi.yaml の `ja`
    pub name: String,
    pub north_star: bool,
    pub direction: KpiDirection,
    /// 集計した時刻に終わる期間（観測を待つ指標は、その日数だけ前に終わる期間）
    pub current: BusinessKpiValue,
    /// `current` の直前の、同じ長さの期間
    pub previous: BusinessKpiValue,
}
