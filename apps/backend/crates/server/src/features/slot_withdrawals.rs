//! コーチの枠の取り下げ（`POST /coaches/me/slots/{slot_id}/withdraw`）。

use application::scheduling::WithdrawSlot;
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::slot_withdrawal_router(
        WithdrawSlot::new(ports.slots(), ports.analytics(), ports.clock()),
        MockTokenService,
    )
}
