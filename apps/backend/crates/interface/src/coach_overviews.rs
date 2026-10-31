//! `/coaches/{coach_id}/overview`。admin-web の `GET /coaches/{coach_id}/overview`（`getCoachOverview`。本部が見るコーチ 1 人の
//! 様子）。本部だけが呼べる。

use std::sync::Arc;

use application::{
    catalog::LessonMenuRepository,
    clock::Clock,
    identity::{AccountRepository, account_id_uuid},
    lesson::{LessonCompletionRepository, MonthlyReviewSummaryLookup},
    profile::{CoachAcceptanceLookup, GetCoachOverview},
    review::LessonReviewRepository,
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Serialize;

use crate::{
    contract::coach_acceptance_status_to_contract,
    error::ApiError,
    lesson_stats::{MonthlyLessonStatsBody, monthly_lesson_stats_body},
};

/// `/coaches/{coach_id}/overview` のハンドラーが使うユースケース。
pub struct CoachOverviewsState<R, A, V, M, W, C, N> {
    pub get_coach_overview: GetCoachOverview<R, A, V, M, W, C, N>,
}

/// admin-web の `CoachOverview`。評価の平均は評価が 0 件なら null。金額は載せない。
#[derive(Serialize)]
pub struct CoachOverviewBody {
    coach_id: String,
    display_name: String,
    acceptance_status: &'static str,
    review_count: u32,
    average_rating: Option<f64>,
    current_month: MonthlyLessonStatsBody,
    previous_month: MonthlyLessonStatsBody,
    published_lesson_menu_count: u32,
}

/// `GET /coaches/{coach_id}/overview`。本部だけが呼べる（役割は `require_role` の層が確かめる）。コーチでない ID・ない ID・
/// UUID でない ID は 404。
#[allow(clippy::type_complexity)]
pub async fn get_coach_overview<R, A, V, M, W, C, N>(
    State(state): State<Arc<CoachOverviewsState<R, A, V, M, W, C, N>>>,
    Path(coach_id): Path<String>,
) -> Result<Json<CoachOverviewBody>, ApiError>
where
    R: AccountRepository,
    A: CoachAcceptanceLookup,
    V: LessonReviewRepository,
    M: LessonCompletionRepository,
    W: MonthlyReviewSummaryLookup,
    C: Clock,
    N: LessonMenuRepository,
{
    // 契約の getCoachOverview では、UUID でない ID はないものとして 404
    let coach_id =
        account_id_uuid::parse(&coach_id).map_err(|_| ApiError::not_found("コーチがない"))?;
    let overview = state
        .get_coach_overview
        .execute(coach_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("コーチがない"))?;
    Ok(Json(CoachOverviewBody {
        coach_id: account_id_uuid::format(coach_id),
        display_name: overview.display_name.as_str().to_string(),
        acceptance_status: coach_acceptance_status_to_contract(overview.acceptance),
        review_count: overview.reviews.review_count(),
        average_rating: overview.reviews.average_rating(),
        current_month: monthly_lesson_stats_body(&overview.lesson_stats.current_month),
        previous_month: monthly_lesson_stats_body(&overview.lesson_stats.previous_month),
        published_lesson_menu_count: overview.published_lesson_menu_count,
    }))
}
