//! `/business-kpis`。admin-web の `GET /business-kpis`（`getBusinessKpis`。事業の KPI の今の期間と前の期間）。本部だけが
//! 呼べる。

use std::sync::Arc;

use application::{
    analytics::{
        BusinessKpi, BusinessKpiValue, GetBusinessKpis, KpiDirection, KpiEventSource, KpiSpecSource,
    },
    clock::Clock,
    identity::TokenVerifier,
};
use axum::{Json, Router, extract::State, routing::get};
use domain::identity::Role;
use serde::Serialize;

use crate::{
    auth::{TokenVerification, require_role},
    business_kpi_text::{business_kpi_presentation, kpi_reason_code, kpi_reason_message},
    contract::date_time_to_contract,
    error::ApiError,
};

/// `/business-kpis` のハンドラーが使うユースケース。
pub struct BusinessKpisState<S, E, C> {
    pub get_business_kpis: GetBusinessKpis<S, E, C>,
}

/// admin-web の `BusinessKpis`。
#[derive(Serialize)]
pub struct BusinessKpisBody {
    computed_at: String,
    kpis: Vec<BusinessKpiBody>,
}

/// admin-web の `BusinessKpi`。`description` は北極星にだけ付ける。
#[derive(Serialize)]
pub struct BusinessKpiBody {
    id: String,
    name: String,
    north_star: bool,
    direction: &'static str,
    unit: &'static str,
    population_label: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'static str>,
    current: BusinessKpiValueBody,
    previous: BusinessKpiValueBody,
}

/// admin-web の `BusinessKpiValue`。値があれば `reason` は null、なければ `value` が null。
#[derive(Serialize)]
pub struct BusinessKpiValueBody {
    window_start: String,
    window_end: String,
    value: Option<f64>,
    reason: Option<BusinessKpiValueReasonBody>,
}

/// admin-web の `BusinessKpiValueReason`。
#[derive(Serialize)]
pub struct BusinessKpiValueReasonBody {
    code: &'static str,
    message: String,
}

fn direction_to_contract(direction: KpiDirection) -> &'static str {
    match direction {
        KpiDirection::UpIsGood => "up_is_good",
        KpiDirection::DownIsGood => "down_is_good",
    }
}

/// 表し方（単位など）を決めていない指標は載せられないので `None`（標準エラー出力に書く）。
fn business_kpi_body(kpi: &BusinessKpi) -> Result<Option<BusinessKpiBody>, ApiError> {
    let Some(presentation) = business_kpi_presentation(&kpi.id) else {
        eprintln!(
            "business_kpis: {} の単位と母集団の名詞が決まっていない",
            kpi.id
        );
        return Ok(None);
    };
    let value = |value: &BusinessKpiValue| -> Result<BusinessKpiValueBody, ApiError> {
        Ok(BusinessKpiValueBody {
            window_start: date_time_to_contract(value.window_start)?,
            window_end: date_time_to_contract(value.window_end)?,
            value: value.value.ok(),
            reason: value.value.err().map(|reason| BusinessKpiValueReasonBody {
                code: kpi_reason_code(reason),
                message: kpi_reason_message(reason, &presentation),
            }),
        })
    };
    Ok(Some(BusinessKpiBody {
        id: kpi.id.clone(),
        name: kpi.name.clone(),
        north_star: kpi.north_star,
        direction: direction_to_contract(kpi.direction),
        unit: presentation.unit,
        population_label: presentation.population_label,
        description: presentation.description,
        current: value(&kpi.current)?,
        previous: value(&kpi.previous)?,
    }))
}

/// `GET /business-kpis`。本部だけが呼べる（役割は `require_role` の層が確かめる）。1 つの指標が数えられなくても
/// 200 で、その指標の値だけが `unavailable`。kpi.yaml を読めない（どの指標も返せない）ときだけ 500。
pub async fn get_business_kpis<S, E, C>(
    State(state): State<Arc<BusinessKpisState<S, E, C>>>,
) -> Result<Json<BusinessKpisBody>, ApiError>
where
    S: KpiSpecSource,
    E: KpiEventSource,
    C: Clock,
{
    let kpis = state
        .get_business_kpis
        .execute()
        .await
        .map_err(ApiError::internal)?;
    let mut bodies = Vec::with_capacity(kpis.kpis.len());
    for kpi in &kpis.kpis {
        bodies.extend(business_kpi_body(kpi)?);
    }
    Ok(Json(BusinessKpisBody {
        computed_at: date_time_to_contract(kpis.computed_at)?,
        kpis: bodies,
    }))
}

/// 事業の KPI（admin-web の `GET /business-kpis`）のルーター。組み立て用のクレートは他の機能のルーターと `merge` する
/// （同じ検証器を渡す）。
///
/// `GET /business-kpis` は本部だけで（コーチ・生徒は 403、トークンがなければ 401）、照会だけで何も保存しない。
pub fn business_kpis_router<S, E, C, T>(
    get_business_kpis: GetBusinessKpis<S, E, C>,
    tokens: T,
) -> Router
where
    S: KpiSpecSource + 'static,
    E: KpiEventSource + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route("/business-kpis", get(self::get_business_kpis::<S, E, C>))
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(BusinessKpisState { get_business_kpis }))
        .layer(TokenVerification::layer(tokens))
}
