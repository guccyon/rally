//! 事業の KPI 1 つの、1 つの期間の値。

use std::time::SystemTime;

use super::KpiUnavailableReason;

/// 事業の KPI 1 つの、1 つの期間（`window_start` を含み `window_end` を含まない）の値。丸めない。
#[derive(Debug, Clone, PartialEq)]
pub struct BusinessKpiValue {
    pub window_start: SystemTime,
    pub window_end: SystemTime,
    pub value: Result<f64, KpiUnavailableReason>,
}
