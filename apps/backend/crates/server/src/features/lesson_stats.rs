//! コーチの今月と先月のレッスンの実績（`GET /coaches/me/lesson-stats`）。
//! 暦月ごとの評価の件数と平均を評価の保存先から読む（`ReviewMonthlyReviewSummaryLookup`）。

use application::{lesson::GetMyLessonStats, review::ReviewMonthlyReviewSummaryLookup};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::lesson_stats_router(
        GetMyLessonStats::new(
            ports.completions(),
            ReviewMonthlyReviewSummaryLookup::new(ports.reviews()),
            ports.clock(),
        ),
        MockTokenService,
    )
}
