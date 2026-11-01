//! 評価へのコーチの返信（`PUT /lesson-reviews/{review_id}/reply`）。

use application::review::ReplyToLessonReview;
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::review_replies_router(
        ReplyToLessonReview::new(ports.reviews(), ports.clock()),
        MockTokenService,
    )
}
