//! 生徒のお気に入りのコーチ（`GET /favorite-coaches`・`PUT`・`DELETE /favorite-coaches/{coach_id}`）。
//! 一覧は評価の件数と平均を評価の保存先から読んで添える。

use application::{
    matching::{AddFavoriteCoach, ListFavoriteCoaches, RemoveFavoriteCoach},
    review::ReviewCoachReviewSummaryLookup,
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::favorite_coaches_router(
        AddFavoriteCoach::new(ports.favorite_coaches(), ports.accounts(), ports.clock()),
        RemoveFavoriteCoach::new(ports.favorite_coaches()),
        ListFavoriteCoaches::new(
            ports.favorite_coaches(),
            ports.accounts(),
            ReviewCoachReviewSummaryLookup::new(ports.reviews()),
        ),
        MockTokenService,
    )
}
