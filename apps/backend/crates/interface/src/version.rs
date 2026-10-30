//! 版を答える `GET /version`（認証なし）。出荷した版の確かめに使う運用のための口で、`/healthz` と同じく
//! 生徒・本部の API の契約（specs/contracts）には載せない。

use std::sync::Arc;

use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;

/// 版が分からないとき（`app_version` がないか空）に返す値。
pub const UNKNOWN_APP_VERSION: &str = "unknown";

/// `GET /version` だけのルーター。`app_version` がないか空（前後の空白だけを含む）なら [`UNKNOWN_APP_VERSION`] を返す。
pub fn router(app_version: Option<&str>) -> Router {
    let app_version = app_version
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(UNKNOWN_APP_VERSION);
    Router::new()
        .route("/version", get(version))
        .with_state(Arc::new(VersionResponse {
            app_version: app_version.to_string(),
        }))
}

#[derive(Serialize)]
struct VersionResponse {
    app_version: String,
}

async fn version(State(response): State<Arc<VersionResponse>>) -> Json<VersionResponse> {
    Json(VersionResponse {
        app_version: response.app_version.clone(),
    })
}
