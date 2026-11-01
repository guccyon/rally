//! `GET /healthz`。死活監視用で、student-app の契約（OpenAPI）には載せない（docs/adr/0003-backend-layers.md の決定 4）。

use axum::{Router, http::StatusCode, routing::get};

/// 死活監視（`GET /healthz`。認証なし）のルーター。組み立て用のクレートは他の機能のルーターと `merge` する。
pub fn healthz_router() -> Router {
    Router::new().route("/healthz", get(healthz))
}

/// 死活監視。プロセスが要求を受け付けられれば 200 を返す。
async fn healthz() -> (StatusCode, &'static str) {
    (StatusCode::OK, "ok")
}
