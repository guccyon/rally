//! KPI の集計に渡す行動イベント 1 件。

use std::collections::BTreeMap;

use super::KpiAttribute;

/// 行動イベント 1 件（events.yaml の `name` と、`required` の属性の名前から値）。式は属性を名前で読むので、
/// 変種ごとの型（[`super::AnalyticsEvent`]）ではなく名前と値の組で持つ。ない属性は null として読む。
#[derive(Debug, Clone, PartialEq)]
pub struct KpiEvent {
    name: String,
    attributes: BTreeMap<String, KpiAttribute>,
}

impl KpiEvent {
    /// 属性のない `name` のイベント（[`Self::with`] で属性を足す）。
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            attributes: BTreeMap::new(),
        }
    }

    /// 属性 `attribute` を `value` にしたイベント。
    pub fn with(mut self, attribute: impl Into<String>, value: impl Into<KpiAttribute>) -> Self {
        self.attributes.insert(attribute.into(), value.into());
        self
    }

    /// events.yaml の `name`。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 属性 `attribute` の値（なければ null）。
    pub fn attribute(&self, attribute: &str) -> &KpiAttribute {
        self.attributes
            .get(attribute)
            .unwrap_or(&KpiAttribute::Null)
    }
}
