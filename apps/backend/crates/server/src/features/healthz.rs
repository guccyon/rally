//! 死活監視（`GET /healthz`。認証なし）。

use axum::Router;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(_ports: &P) -> Router {
    interface::healthz_router()
}
