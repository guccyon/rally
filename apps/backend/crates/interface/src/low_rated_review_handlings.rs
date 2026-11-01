//! 低い評価への本部の対応済みの印。admin-web の `PUT /lesson-reviews/{review_id}/handling`
//! （`markLowRatedLessonReviewHandled`。本部だけ）。2 回目は何も変えず、最初の印を返す。
//! 印は `listLowRatedLessonReviews` の評価 1 件ごとに載り、一覧は `handled` で絞れる（[`crate::low_rated_reviews`]）。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::{TokenVerifier, account_id_uuid},
    review::{
        LessonReviewRepository, MarkLowRatedReviewHandled, MarkLowRatedReviewHandledCommand,
        MarkLowRatedReviewHandledError, lesson_review_id_uuid,
    },
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::put,
};
use domain::{identity::Role, review::LowRatedReviewHandling};
use serde::Serialize;

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    contract::date_time_to_contract,
    error::ApiError,
};

/// `/lesson-reviews/{review_id}/handling` のハンドラーが使うユースケース。
pub struct LowRatedReviewHandlingsState<R, C> {
    pub mark_low_rated_review_handled: MarkLowRatedReviewHandled<R, C>,
}

/// ハンドラーが共有する [`LowRatedReviewHandlingsState`]。
type Shared<R, C> = Arc<LowRatedReviewHandlingsState<R, C>>;

/// admin-web の `LowRatedReviewHandling`。
#[derive(Serialize)]
pub struct LowRatedReviewHandlingBody {
    handled_by: String,
    handled_at: String,
}

/// 対応済みの印 `handling` を admin-web の `LowRatedReviewHandling` にする（印を付ける操作と低い評価の一覧で共有する）。
pub(crate) fn low_rated_review_handling_body(
    handling: &LowRatedReviewHandling,
) -> Result<LowRatedReviewHandlingBody, ApiError> {
    Ok(LowRatedReviewHandlingBody {
        handled_by: account_id_uuid::format(handling.handled_by()),
        handled_at: date_time_to_contract(handling.handled_at())?,
    })
}

/// `PUT /lesson-reviews/{review_id}/handling`（admin-web）。本部だけが呼べ（役割は `require_role` の層が確かめる）、
/// 印を付けるのはトークンのアカウント。リクエストの本文は読まない。低い評価かはユースケース（domain）が確かめる。
pub async fn mark_low_rated_review_handled<R, C>(
    State(state): State<Shared<R, C>>,
    account: AuthenticatedAccount,
    Path(review_id): Path<String>,
) -> Result<Json<LowRatedReviewHandlingBody>, ApiError>
where
    R: LessonReviewRepository,
    C: Clock,
{
    // UUID でない ID はないものとして扱う
    let review_id = lesson_review_id_uuid::parse(&review_id).map_err(|_| review_not_found())?;
    let review = state
        .mark_low_rated_review_handled
        .execute(MarkLowRatedReviewHandledCommand {
            account_id: account.account_id(),
            review_id,
        })
        .await
        .map_err(|error| match error {
            MarkLowRatedReviewHandledError::ReviewNotFound => review_not_found(),
            MarkLowRatedReviewHandledError::Invalid(error) => ApiError::from(error),
            MarkLowRatedReviewHandledError::Unavailable(error) => ApiError::internal(error),
        })?;
    let handling = review
        .handling()
        .ok_or_else(|| ApiError::internal("印を付けた評価に印がない"))?;
    Ok(Json(low_rated_review_handling_body(handling)?))
}

fn review_not_found() -> ApiError {
    ApiError::not_found("評価がない")
}

/// 低い評価への本部の対応済みの印（admin-web の `PUT /lesson-reviews/{review_id}/handling`）のルーター。
/// 組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// 本部だけ（コーチ・生徒は 403）。
pub fn low_rated_review_handlings_router<R, C, T>(
    mark_low_rated_review_handled: MarkLowRatedReviewHandled<R, C>,
    tokens: T,
) -> Router
where
    R: LessonReviewRepository + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/lesson-reviews/{review_id}/handling",
            put(self::mark_low_rated_review_handled::<R, C>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(LowRatedReviewHandlingsState {
            mark_low_rated_review_handled,
        }))
        .layer(TokenVerification::layer(tokens))
}
