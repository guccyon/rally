//! 低い評価への本部の対応済みの印（`PUT /lesson-reviews/{review_id}/handling`）。

use application::review::MarkLowRatedReviewHandled;
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::low_rated_review_handlings_router(
        MarkLowRatedReviewHandled::new(ports.reviews(), ports.clock()),
        MockTokenService,
    )
}
