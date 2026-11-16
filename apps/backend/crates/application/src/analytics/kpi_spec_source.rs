//! KPI の式の正本（specs/analytics/kpi.yaml）を読むポート。

use std::future::Future;

use crate::Unavailable;

/// specs/analytics/kpi.yaml の中身（YAML の文字列）を読むポート。式はこのファイルの 1 か所にだけあり、
/// バックエンドに写さない（kpi.yaml の冒頭）ので、照会のたびに正本を読む。読めなければ [`Unavailable`]。
pub trait KpiSpecSource: Send + Sync {
    fn kpi_yaml(&self) -> impl Future<Output = Result<String, Unavailable>> + Send;
}
