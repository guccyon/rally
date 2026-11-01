//! 本部が見る、プロフィールに足りない項目のあるコーチの一覧（`GET /coaches/incomplete-profiles`）。コーチのアカウントと
//! 自己紹介文を、どちらもアカウントの保存先（当面は identity の表）から読む。

use application::profile::ListIncompleteCoachProfiles;
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::incomplete_coach_profiles_router(
        ListIncompleteCoachProfiles::new(ports.accounts(), ports.accounts()),
        MockTokenService,
    )
}
