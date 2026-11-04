//! スポーツごとの予約枠の需給（`GET /slot-supply`）。予約の保存先から期間の枠と予約を読み、競技で絞るときはコーチの
//! 提供スポーツをアカウントの保存先から読む。

use application::{profile::ProfileCoachAccountLookup, scheduling::GetSlotSupply};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::slot_supply_router(
        GetSlotSupply::new(
            ports.bookings(),
            ProfileCoachAccountLookup::new(ports.accounts()),
            ports.clock(),
        ),
        MockTokenService,
    )
}
