//! 事業の KPI（admin-web の `getBusinessKpis`）の、契約の表し方と画面にそのまま出す文言（単位・母集団の名詞・見方の 1 文・
//! 値が出せない理由の文）。kpi.yaml にない表示の決まりはここだけで決める（画面は組み立てない）。式は持たない。

use application::analytics::KpiUnavailableReason;

/// 指標ごとの表し方（kpi.yaml の `id` ごと）。
pub(crate) struct BusinessKpiPresentation {
    /// `BusinessKpi.unit`
    pub(crate) unit: &'static str,
    /// `BusinessKpi.population_label`
    pub(crate) population_label: &'static str,
    /// `BusinessKpi.description`（北極星にだけ付ける）
    pub(crate) description: Option<&'static str>,
    /// 群ごとに数える指標の、群の件数に数えるものの名詞（理由 `below_group_minimum` の文に使う）
    pub(crate) group_count_label: &'static str,
}

/// kpi.yaml の `id` の表し方。知らない `id` は `None`（kpi.yaml に足したらここにも足す。試験が見張る）。
pub(crate) fn business_kpi_presentation(id: &str) -> Option<BusinessKpiPresentation> {
    let (unit, population_label, description, group_count_label) = match id {
        "first_lesson_retention_1m" => (
            "ratio",
            "生徒",
            Some("初回のレッスンを受けた生徒が、月額で続けているかを見る事業の北極星です。"),
            "件数",
        ),
        "coach_retention_variance" => ("variance", "コーチ", None, "初回レッスン"),
        "slot_fill_rate" => ("ratio", "枠", None, "件数"),
        "call_quality_score" => ("score", "通話", None, "件数"),
        "video_review_lead_time" => ("hours", "動画", None, "件数"),
        "days_to_first_lesson" => ("days", "体験予約", None, "件数"),
        _ => return None,
    };
    Some(BusinessKpiPresentation {
        unit,
        population_label,
        description,
        group_count_label,
    })
}

/// 理由の種類（`BusinessKpiValueReason.code`）。
pub(crate) fn kpi_reason_code(reason: KpiUnavailableReason) -> &'static str {
    match reason {
        KpiUnavailableReason::NoPopulation => "no_population",
        KpiUnavailableReason::BelowGroupMinimum { .. } => "below_group_minimum",
        KpiUnavailableReason::NotObservableYet { .. } => "not_observable_yet",
        KpiUnavailableReason::Unavailable => "unavailable",
    }
}

/// 画面にそのまま出す理由の文（`BusinessKpiValueReason.message`）。
pub(crate) fn kpi_reason_message(
    reason: KpiUnavailableReason,
    presentation: &BusinessKpiPresentation,
) -> String {
    let label = presentation.population_label;
    match reason {
        KpiUnavailableReason::NoPopulation => {
            format!("この期間に数える{label}がないため、出せません。")
        }
        KpiUnavailableReason::BelowGroupMinimum { min_count } => format!(
            "{}が {min_count} 件以上の{label}がいないため、出せません。",
            presentation.group_count_label
        ),
        KpiUnavailableReason::NotObservableYet { observe_days } => {
            format!("{observe_days} 日目まで見終えた{label}がまだいないため、出せません。")
        }
        KpiUnavailableReason::Unavailable => {
            "集計に失敗したため、出せません。時間をおいて開き直してください。".to_string()
        }
    }
}
