//! KPI の良い向き。

/// KPI の良い向き（kpi.yaml の `direction`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KpiDirection {
    /// `up_is_good`: 高いほど良い
    UpIsGood,
    /// `down_is_good`: 低いほど良い
    DownIsGood,
}
