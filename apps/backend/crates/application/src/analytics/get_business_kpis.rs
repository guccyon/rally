//! 本部が見る事業の KPI の今の期間と前の期間（admin-web の `GET /business-kpis`）。

use std::{
    collections::{BTreeMap, BTreeSet},
    time::SystemTime,
};

use super::{
    BusinessKpi, BusinessKpiValue, BusinessKpis, KpiDefinition, KpiEventSource, KpiSpec,
    KpiSpecSource, KpiUnavailableReason,
    kpi_evaluation::{Evaluation, Failure, days_before},
};
use crate::{Unavailable, clock::Clock};

/// specs/analytics/kpi.yaml の KPI を、式（`formula`）と期間（`window`・`window_on`）のとおりに数える。式は照会のたびに
/// 正本（[`KpiSpecSource`]）から読み、ここにも他の場所にも写さない。
///
/// - `current` は集計した時刻（式の `now`）に終わる `window` の期間、`previous` はその直前の同じ長さの期間。母集団の
///   行から先を見る式（`f.occurred_at + days(60)` など）は、その日数を観測し終えた行だけを数えるため、両方の期間を
///   その日数だけ前にずらす（`variance` は群ごとに計算する KPI の分もずらす）
/// - 値が出せない期間は理由（[`KpiUnavailableReason`]）を返す。式が読めない・参照するイベントを読めない・評価に
///   失敗した指標は、その指標だけを [`KpiUnavailableReason::Unavailable`] にし、ほかの指標はそのまま返す
/// - kpi.yaml そのものを読めない（どの指標も返せない）ときだけ [`Unavailable`]
pub struct GetBusinessKpis<S, E, C> {
    spec: S,
    events: E,
    clock: C,
}

impl<S, E, C> GetBusinessKpis<S, E, C>
where
    S: KpiSpecSource,
    E: KpiEventSource,
    C: Clock,
{
    pub fn new(spec: S, events: E, clock: C) -> Self {
        Self {
            spec,
            events,
            clock,
        }
    }

    pub async fn execute(&self) -> Result<BusinessKpis, Unavailable> {
        let now = self.clock.now();
        let spec = KpiSpec::parse(&self.spec.kpi_yaml().await?).map_err(Unavailable::new)?;

        let names: BTreeSet<String> = spec
            .kpis
            .iter()
            .flat_map(|kpi| spec.required_events(&kpi.id))
            .collect();
        let mut events = BTreeMap::new();
        for name in names {
            match self.events.events(&name).await {
                Ok(loaded) => {
                    events.insert(name, loaded);
                }
                Err(error) => eprintln!("business_kpis: {name} を読めない: {error}"),
            }
        }

        let evaluation = Evaluation {
            spec: &spec,
            events: &events,
            now,
        };
        let kpis = spec
            .kpis
            .iter()
            .map(|kpi| business_kpi(&evaluation, kpi, now))
            .collect();
        Ok(BusinessKpis {
            computed_at: now,
            kpis,
        })
    }
}

fn business_kpi(evaluation: &Evaluation<'_>, kpi: &KpiDefinition, now: SystemTime) -> BusinessKpi {
    let lookahead_days = evaluation.spec.lookahead_days(&kpi.id);
    let missing: Vec<String> = evaluation
        .spec
        .required_events(&kpi.id)
        .into_iter()
        .filter(|name| !evaluation.events.contains_key(name))
        .collect();
    let current_end = days_before(now, lookahead_days);
    let current_start = current_end - kpi.window;
    let previous_start = current_start - kpi.window;
    let value = |start: SystemTime, end: SystemTime| {
        let value = match &kpi.formula {
            Ok(_) if !missing.is_empty() => Err(Failure::Error(format!(
                "参照するイベントを読めない: {missing:?}"
            ))),
            Ok(windowed) => evaluation.period(windowed, lookahead_days, start, end),
            Err(error) => Err(Failure::Error(error.clone())),
        };
        BusinessKpiValue {
            window_start: start,
            window_end: end,
            value: value.map_err(|failure| match failure {
                Failure::Reason(reason) => reason,
                Failure::Error(error) => {
                    eprintln!("business_kpis: {} を数えられない: {error}", kpi.id);
                    KpiUnavailableReason::Unavailable
                }
            }),
        }
    };
    BusinessKpi {
        id: kpi.id.clone(),
        name: kpi.name.clone(),
        north_star: kpi.north_star,
        direction: kpi.direction,
        current: value(current_start, current_end),
        previous: value(previous_start, current_start),
    }
}
