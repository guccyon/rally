//! KPI の集計に渡す行動イベントの属性の値。

use std::time::SystemTime;

/// 行動イベント（specs/analytics/events.yaml）の属性 1 つの値。式（specs/analytics/kpi.yaml）が比べ・計算できる形に
/// 揃える。ID は文字列（UUID のハイフン付きの形）で持ち、同じ ID どうしを `=` で比べる。
#[derive(Debug, Clone, PartialEq)]
pub enum KpiAttribute {
    /// 値が分からない（保存先に残っていない属性）。式の中では null として扱う
    Null,
    Bool(bool),
    Number(f64),
    Time(SystemTime),
    Text(String),
}

impl From<bool> for KpiAttribute {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<f64> for KpiAttribute {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<SystemTime> for KpiAttribute {
    fn from(value: SystemTime) -> Self {
        Self::Time(value)
    }
}

impl From<String> for KpiAttribute {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for KpiAttribute {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl<T: Into<KpiAttribute>> From<Option<T>> for KpiAttribute {
    fn from(value: Option<T>) -> Self {
        value.map_or(Self::Null, Into::into)
    }
}
