//! 事業の KPI（`GET /business-kpis`）。式は照会のたびに kpi.yaml（`KPI_SPEC_PATH`。なければリポジトリの中）から読み、
//! 行動イベントは KPI の集計のイベントの保存先から読む。

use application::analytics::GetBusinessKpis;
use axum::Router;
use infra::FileKpiSpec;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::business_kpis_router(
        GetBusinessKpis::new(FileKpiSpec::from_env(), ports.kpi_events(), ports.clock()),
        ports.tokens(),
    )
}
