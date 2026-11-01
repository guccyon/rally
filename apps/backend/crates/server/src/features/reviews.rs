//! レッスンの評価（`POST /lesson-reviews`）とコーチの評価（`GET /coaches/{coach_id}/review-summary`・`/reviews`）。
//! 評価するレッスンは完了記録の保存先から読み（`LessonCompletionLookup`）、
//! 評価の一覧に添えるメニューは完了記録の元が持つ写しを読む（`LessonCompletionMenuLookup`）。

use application::{
    lesson::{LessonCompletionLookup, LessonCompletionMenuLookup},
    review::{GetCoachReviewSummary, ListCoachReviews, SubmitLessonReview},
    scheduling::SchedulingBookingLookup,
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::reviews_router(
        SubmitLessonReview::new(
            LessonCompletionLookup::new(ports.completions()),
            ports.reviews(),
            ports.analytics(),
            ports.clock(),
        ),
        GetCoachReviewSummary::new(ports.reviews()),
        ListCoachReviews::new(
            ports.reviews(),
            LessonCompletionMenuLookup::new(
                ports.completions(),
                ports.sessions(),
                SchedulingBookingLookup::new(ports.bookings()),
                ports.videos(),
            ),
        ),
        MockTokenService,
    )
}
