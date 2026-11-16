//! kpi.yaml の読み取り。

use std::{collections::BTreeSet, time::Duration};

use serde_yaml::Value;

use super::{KpiDefinition, KpiDirection, KpiFormula, KpiWindowedFormula};

/// specs/analytics/kpi.yaml の `kpis`（並びはファイルの順）。
#[derive(Debug, Clone, PartialEq)]
pub struct KpiSpec {
    pub kpis: Vec<KpiDefinition>,
}

impl KpiSpec {
    /// `yaml`（kpi.yaml の中身）を読む。`kpis` の並びや、各項目の `id`・`ja`・`direction` が読めなければ、どの指標も
    /// 返せないので理由の文（`window` も同じ）。式が読めない項目は、その項目の [`KpiDefinition::formula`] に理由を残す。
    pub fn parse(yaml: &str) -> Result<Self, String> {
        let root: Value = serde_yaml::from_str(yaml).map_err(|e| format!("kpi.yaml: {e}"))?;
        let items = root["kpis"]
            .as_sequence()
            .ok_or("kpi.yaml に kpis の並びがない")?;
        let kpis = items
            .iter()
            .map(definition)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { kpis })
    }

    /// `id` の定義。
    pub fn get(&self, id: &str) -> Option<&KpiDefinition> {
        self.kpis.iter().find(|kpi| kpi.id == id)
    }

    /// `id` の指標の母集団が観測を待つ日数（[`KpiFormula::lookahead_days`]。`variance` は群ごとに計算する KPI の分も
    /// 含める）。式が読めなければ 0。
    pub fn lookahead_days(&self, id: &str) -> u32 {
        self.lookahead_days_within(id, 0)
    }

    /// `id` の指標が参照するイベント名（`variance` は群ごとに計算する KPI の分も含める）。式が読めなければ空。
    pub fn required_events(&self, id: &str) -> BTreeSet<String> {
        let mut events = BTreeSet::new();
        let mut next = Some(id);
        for _ in 0..=self.kpis.len() {
            let Some(Ok(windowed)) = next.and_then(|id| self.get(id)).map(|kpi| &kpi.formula)
            else {
                break;
            };
            events.extend(windowed.formula.events());
            next = windowed.formula.referenced_kpi();
        }
        events
    }

    fn lookahead_days_within(&self, id: &str, depth: usize) -> u32 {
        let Some(Ok(windowed)) = self.get(id).map(|kpi| &kpi.formula) else {
            return 0;
        };
        let own = windowed.formula.lookahead_days(&windowed.window_on);
        match windowed.formula.referenced_kpi() {
            Some(referenced) if depth < self.kpis.len() => {
                own.max(self.lookahead_days_within(referenced, depth + 1))
            }
            _ => own,
        }
    }
}

fn text<'a>(item: &'a Value, key: &str) -> Result<&'a str, String> {
    item[key]
        .as_str()
        .ok_or_else(|| format!("kpi.yaml の項目 {:?} に文字列の {key} がない", item["id"]))
}

fn definition(item: &Value) -> Result<KpiDefinition, String> {
    let id = text(item, "id")?.to_string();
    let name = text(item, "ja")?.to_string();
    let direction = match text(item, "direction")? {
        "up_is_good" => KpiDirection::UpIsGood,
        "down_is_good" => KpiDirection::DownIsGood,
        other => return Err(format!("kpi.yaml の {id} の direction が読めない: {other}")),
    };
    let window = text(item, "window")?;
    let days: u64 = window
        .strip_suffix('d')
        .and_then(|days| days.parse().ok())
        .filter(|days| *days > 0)
        .ok_or_else(|| format!("kpi.yaml の {id} の window は <日数>d: {window}"))?;
    Ok(KpiDefinition {
        north_star: item["north_star"].as_bool().unwrap_or(false),
        window: Duration::from_secs(days * 24 * 60 * 60),
        formula: windowed_formula(item).map_err(|e| format!("kpi.yaml の {id}: {e}")),
        id,
        name,
        direction,
    })
}

fn windowed_formula(item: &Value) -> Result<KpiWindowedFormula, String> {
    let formula = KpiFormula::parse(text(item, "formula")?)?;
    let window_on = text(item, "window_on")?;
    let (alias, attribute) = window_on
        .split_once('.')
        .ok_or_else(|| format!("window_on は <別名>.<属性>: {window_on}"))?;
    if alias != formula.population_alias() {
        return Err(format!(
            "window_on の別名 {alias} が母集団の別名 {} と違う",
            formula.population_alias()
        ));
    }
    Ok(KpiWindowedFormula {
        window_on: attribute.to_string(),
        formula,
    })
}
