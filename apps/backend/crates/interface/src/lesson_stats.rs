//! `/coaches/me/lesson-stats`。admin-web の `GET /coaches/me/lesson-stats`（`getMyLessonStats`。コーチの今月と先月の
//! レッスンの実績）。コーチだけが呼べる。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::{TokenVerifier, account_id_uuid},
    lesson::{
        GetMyLessonStats, LessonCompletionRepository, MonthlyLessonStats,
        MonthlyReviewSummaryLookup,
    },
};
use axum::{Json, Router, extract::State, routing::get};
use domain::identity::Role;
use serde::Serialize;

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    contract::year_month_to_contract,
    error::ApiError,
};

/// `/coaches/me/lesson-stats` のハンドラーが使うユースケース。
pub struct LessonStatsState<M, R, C> {
    pub get_my_lesson_stats: GetMyLessonStats<M, R, C>,
}

/// admin-web の `CoachLessonStats`。
#[derive(Serialize)]
pub struct CoachLessonStatsBody {
    coach_id: String,
    current_month: MonthlyLessonStatsBody,
    previous_month: MonthlyLessonStatsBody,
}

/// admin-web の `MonthlyLessonStats`。評価の平均は評価が 0 件なら null。金額は載せない。
#[derive(Serialize)]
pub struct MonthlyLessonStatsBody {
    month: String,
    live_lesson_count: u32,
    async_lesson_count: u32,
    review_count: u32,
    average_rating: Option<f64>,
}

fn monthly_lesson_stats_body(stats: &MonthlyLessonStats) -> MonthlyLessonStatsBody {
    MonthlyLessonStatsBody {
        month: year_month_to_contract(stats.month),
        live_lesson_count: stats.lessons.live_count(),
        async_lesson_count: stats.lessons.async_count(),
        review_count: stats.reviews.review_count(),
        average_rating: stats.reviews.average_rating(),
    }
}

/// `GET /coaches/me/lesson-stats`。コーチだけが呼べる（役割は `require_role` の層が確かめる）。照会するのはトークンの
/// コーチ自身の実績だけ。
pub async fn get_my_lesson_stats<M, R, C>(
    State(state): State<Arc<LessonStatsState<M, R, C>>>,
    account: AuthenticatedAccount,
) -> Result<Json<CoachLessonStatsBody>, ApiError>
where
    M: LessonCompletionRepository,
    R: MonthlyReviewSummaryLookup,
    C: Clock,
{
    let stats = state
        .get_my_lesson_stats
        .execute(account.account_id())
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(CoachLessonStatsBody {
        coach_id: account_id_uuid::format(account.account_id()),
        current_month: monthly_lesson_stats_body(&stats.current_month),
        previous_month: monthly_lesson_stats_body(&stats.previous_month),
    }))
}

/// コーチの今月と先月のレッスンの実績（admin-web の `GET /coaches/me/lesson-stats`）のルーター。
/// 組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// `GET /coaches/me/lesson-stats` はコーチだけで（本部・生徒は 403）、照会するのはトークンのコーチ自身の実績だけ。
pub fn lesson_stats_router<M, R, C, T>(
    get_my_lesson_stats: GetMyLessonStats<M, R, C>,
    tokens: T,
) -> Router
where
    M: LessonCompletionRepository + 'static,
    R: MonthlyReviewSummaryLookup + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/coaches/me/lesson-stats",
            get(self::get_my_lesson_stats::<M, R, C>),
        )
        .route_layer(require_role(&[Role::Coach]))
        .with_state(Arc::new(LessonStatsState {
            get_my_lesson_stats,
        }))
        .layer(TokenVerification::layer(tokens))
}
