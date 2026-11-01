//! スポーツごとの予約枠の需給（`GET /slot-supply`）。予約の保存先から期間の枠と予約を読む。

use application::scheduling::GetSlotSupply;
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::slot_supply_router(
        GetSlotSupply::new(ports.bookings(), ports.clock()),
        MockTokenService,
    )
}
