//! 本部が見る低い評価の一覧（`GET /lesson-reviews/low`）。メニューは完了記録の元が持つ写しを、
//! コーチ・生徒の表示名はアカウントの保存先から読んで添える。スポーツで絞るときは、評価したレッスンのスポーツを
//! 完了記録の元（予約の枠・動画）から読む（`LessonCompletionSportLookup`）。

use application::{
    lesson::{LessonCompletionMenuLookup, LessonCompletionSportLookup},
    profile::{ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup},
    review::ListLowRatedReviews,
    scheduling::SchedulingBookingLookup,
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::low_rated_reviews_router(
        ListLowRatedReviews::new(
            ports.reviews(),
            LessonCompletionMenuLookup::new(
                ports.completions(),
                ports.sessions(),
                SchedulingBookingLookup::new(ports.bookings()),
                ports.videos(),
            ),
            LessonCompletionSportLookup::new(
                ports.completions(),
                ports.sessions(),
                SchedulingBookingLookup::new(ports.bookings()),
                ports.videos(),
            ),
            ProfileCoachDisplayNameLookup::new(ports.accounts()),
            ProfileStudentDisplayNameLookup::new(ports.accounts()),
            ports.clock(),
        ),
        MockTokenService,
    )
}
