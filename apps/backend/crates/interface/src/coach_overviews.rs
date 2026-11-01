//! `/coaches/{coach_id}/overview`。admin-web の `GET /coaches/{coach_id}/overview`（`getCoachOverview`。本部が見るコーチ 1 人の
//! 様子）。本部だけが呼べる。

use std::sync::Arc;

use application::{
    catalog::LessonMenuRepository,
    clock::Clock,
    identity::{AccountRepository, TokenVerifier, account_id_uuid},
    lesson::{LessonCompletionRepository, MonthlyReviewSummaryLookup},
    profile::{CoachAcceptanceLookup, GetCoachOverview},
    review::LessonReviewRepository,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use domain::identity::Role;
use serde::Serialize;

use crate::{
    auth::{TokenVerification, require_role},
    contract::coach_acceptance_status_to_contract,
    error::ApiError,
    lesson_stats::{MonthlyLessonStatsBody, monthly_lesson_stats_body},
};

/// `/coaches/{coach_id}/overview` のハンドラーが使うユースケース。
pub struct CoachOverviewsState<R, A, V, M, W, C, N> {
    pub get_coach_overview: GetCoachOverview<R, A, V, M, W, C, N>,
}

/// admin-web の `CoachOverview`。評価の平均は評価が 0 件なら null。直近の低い評価の件数と、そのうち本部が未対応の件数を
/// 添える。金額は載せない。
#[derive(Serialize)]
pub struct CoachOverviewBody {
    coach_id: String,
    display_name: String,
    acceptance_status: &'static str,
    review_count: u32,
    average_rating: Option<f64>,
    low_rated_review_count: u32,
    unhandled_low_rated_review_count: u32,
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
        low_rated_review_count: overview.low_rated_reviews.low_rated_count(),
        unhandled_low_rated_review_count: overview.low_rated_reviews.unhandled_count(),
        current_month: monthly_lesson_stats_body(&overview.lesson_stats.current_month),
        previous_month: monthly_lesson_stats_body(&overview.lesson_stats.previous_month),
        published_lesson_menu_count: overview.published_lesson_menu_count,
    }))
}

/// 本部が見るコーチ 1 人の様子（admin-web の `GET /coaches/{coach_id}/overview`）のルーター。組み立て用のクレートは
/// 他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// 本部だけ（コーチ・生徒は 403）。
#[allow(clippy::type_complexity)]
pub fn coach_overview_router<R, A, V, M, W, C, N, T>(
    get_coach_overview: GetCoachOverview<R, A, V, M, W, C, N>,
    tokens: T,
) -> Router
where
    R: AccountRepository + 'static,
    A: CoachAcceptanceLookup + 'static,
    V: LessonReviewRepository + 'static,
    M: LessonCompletionRepository + 'static,
    W: MonthlyReviewSummaryLookup + 'static,
    C: Clock + 'static,
    N: LessonMenuRepository + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/coaches/{coach_id}/overview",
            get(self::get_coach_overview::<R, A, V, M, W, C, N>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(CoachOverviewsState { get_coach_overview }))
        .layer(TokenVerification::layer(tokens))
}
