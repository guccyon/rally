//! 本部が見るコーチ 1 人の様子（`GET /coaches/{coach_id}/overview`）。受付の状態は scheduling に問い合わせ
//! （`SchedulingCoachAcceptanceLookup`）、評価の件数と平均・月ごとのレッスンの数・メニューはそれぞれの保存先から読む。

use application::{
    catalog::ListCoachLessonMenus,
    lesson::GetMyLessonStats,
    profile::GetCoachOverview,
    review::{GetCoachReviewSummary, ReviewMonthlyReviewSummaryLookup},
    scheduling::SchedulingCoachAcceptanceLookup,
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::coach_overview_router(
        GetCoachOverview::new(
            ports.accounts(),
            SchedulingCoachAcceptanceLookup::new(ports.coach_acceptances()),
            GetCoachReviewSummary::new(ports.reviews()),
            GetMyLessonStats::new(
                ports.completions(),
                ReviewMonthlyReviewSummaryLookup::new(ports.reviews()),
                ports.clock(),
            ),
            ListCoachLessonMenus::new(ports.lesson_menus()),
        ),
        MockTokenService,
    )
}
