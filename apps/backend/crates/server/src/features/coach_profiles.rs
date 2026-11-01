//! 生徒が見るコーチの紹介（`GET /coaches/{coach_id}`）。
//! お気に入りかをお気に入りの保存先から読む（`MatchingCoachFavoriteLookup`）。

use application::{matching::MatchingCoachFavoriteLookup, profile::GetCoachProfile};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::coach_profile_router(
        GetCoachProfile::new(
            ports.accounts(),
            MatchingCoachFavoriteLookup::new(ports.favorite_coaches()),
        ),
        MockTokenService,
    )
}
