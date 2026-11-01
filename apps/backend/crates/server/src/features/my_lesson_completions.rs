//! 生徒が受けたレッスンの一覧（`GET /lessons/completions/me`）。メニューは完了記録の元が持つ写しを、コーチの表示名は
//! `ProfileCoachDisplayNameLookup` で、評価を付けたかは評価の保存先から読む。

use application::{
    lesson::ListMyLessonCompletions, profile::ProfileCoachDisplayNameLookup,
    review::ReviewLessonCompletionReviewLookup, scheduling::SchedulingBookingLookup,
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::my_lesson_completions_router(
        ListMyLessonCompletions::new(
            ports.completions(),
            ports.sessions(),
            SchedulingBookingLookup::new(ports.bookings()),
            ports.videos(),
            ProfileCoachDisplayNameLookup::new(ports.accounts()),
            ReviewLessonCompletionReviewLookup::new(ports.reviews()),
        ),
        MockTokenService,
    )
}
