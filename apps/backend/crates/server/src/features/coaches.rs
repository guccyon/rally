//! コーチの一覧（`GET /coaches`。本部と生徒）。受付の状態は scheduling
//! に問い合わせて添える・除く（`SchedulingCoachAcceptanceLookup`）。本部の一覧の直近 30 日に終わったレッスンの数は
//! 完了記録の保存先から読む（`LessonCoachRecentLessonCountLookup`）。

use application::{
    catalog::CatalogPublishedLessonMenuCountLookup,
    lesson::LessonCoachRecentLessonCountLookup,
    matching::ListCoachesForStudent,
    profile::{ListCoaches, ProfileCoachCandidateLookup, ProfileStudentSportLookup},
    review::ReviewCoachReviewSummaryLookup,
    scheduling::SchedulingCoachAcceptanceLookup,
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::coaches_router(
        ListCoaches::new(
            ports.accounts(),
            SchedulingCoachAcceptanceLookup::new(ports.coach_acceptances()),
            LessonCoachRecentLessonCountLookup::new(ports.completions(), ports.clock()),
        ),
        ListCoachesForStudent::new(
            ProfileStudentSportLookup::new(ports.accounts()),
            ProfileCoachCandidateLookup::new(ports.accounts()),
            ReviewCoachReviewSummaryLookup::new(ports.reviews()),
            CatalogPublishedLessonMenuCountLookup::new(ports.lesson_menus()),
            ports.favorite_coaches(),
            SchedulingCoachAcceptanceLookup::new(ports.coach_acceptances()),
        ),
        MockTokenService,
    )
}
