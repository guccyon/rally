//! `/coaches`。admin-web の `GET /coaches`（`listCoaches`）は本部だけで、コーチを受付の状態を添えて返す（任意の
//! `acceptance_status`・`sport` で絞る）。student-app の
//! `GET /coaches`（`listCoachesForStudent`）は生徒だけで、スポーツ（省いたら生徒自身のスポーツ）を提供するコーチを、
//! 評価の件数と平均と公開中のレッスンメニューの数と、生徒のお気に入りに入っているかを添えて返す（ゴルフは golf-app の範囲なので
//! 載せない。受付を止めているコーチも載せない）。
//! 同じパスなので、どちらの形で返すかはトークンの役割で決める。

use std::sync::Arc;

use application::{
    identity::{AccountRepository, TokenVerifier, account_id_uuid},
    matching::{
        CoachCandidateLookup, CoachListing, CoachReviewSummaryLookup, FavoriteCoachRepository,
        ListCoachesForStudent, ListCoachesForStudentCommand, ListCoachesForStudentError,
        PublishedLessonMenuCountLookup, StudentSportLookup, SuspendedCoachLookup,
    },
    profile::{CoachAcceptanceLookup, ListCoaches, ListedCoach},
};
use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    response::{IntoResponse, Response},
    routing::get,
};
use domain::{catalog::Sport, identity::Role};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{self, AuthenticatedAccount, TokenVerification, require_role},
    contract::{
        admin_web_sport, coach_acceptance_status_from_contract,
        coach_acceptance_status_to_contract, sport_to_contract, student_app_sport,
    },
    error::ApiError,
};

/// `/coaches` のハンドラーが使うユースケース。
pub struct CoachesState<R, A, S, C, V, M, F, U> {
    pub list_coaches: ListCoaches<R, A>,
    pub list_coaches_for_student: ListCoachesForStudent<S, C, V, M, F, U>,
}

/// ハンドラーが共有する [`CoachesState`]。
type Shared<R, A, S, C, V, M, F, U> = Arc<CoachesState<R, A, S, C, V, M, F, U>>;

/// admin-web の `CoachList`。
#[derive(Serialize)]
pub struct CoachListBody {
    coaches: Vec<CoachBody>,
}

/// admin-web の `Coach`。
#[derive(Serialize)]
pub struct CoachBody {
    coach_id: String,
    display_name: String,
    acceptance_status: &'static str,
}

/// admin-web の `listCoaches` のクエリ。
#[derive(Deserialize)]
pub struct ListCoachesParams {
    acceptance_status: Option<String>,
    sport: Option<String>,
}

/// student-app の `listCoachesForStudent` のクエリ。
#[derive(Deserialize)]
pub struct ListCoachesForStudentParams {
    sport: Option<String>,
}

/// student-app の `CoachListingList`。
#[derive(Serialize)]
pub struct CoachListingListBody {
    coaches: Vec<CoachListingBody>,
}

/// student-app の `CoachListing`。
#[derive(Serialize)]
pub struct CoachListingBody {
    coach_id: String,
    display_name: String,
    sport: &'static str,
    review_count: u32,
    average_rating: Option<f64>,
    published_lesson_menu_count: u32,
    is_favorite: bool,
}

/// `GET /coaches`。本部（admin-web の `listCoaches`）と生徒（student-app の `listCoachesForStudent`）が呼べる
/// （役割は `require_role` の層が確かめる。コーチは 403）。
#[allow(clippy::type_complexity)]
pub async fn list_coaches<R, A, S, C, V, M, F, U>(
    State(state): State<Shared<R, A, S, C, V, M, F, U>>,
    account: AuthenticatedAccount,
    headquarters_params: Result<Query<ListCoachesParams>, QueryRejection>,
    params: Result<Query<ListCoachesForStudentParams>, QueryRejection>,
) -> Result<Response, ApiError>
where
    R: AccountRepository,
    A: CoachAcceptanceLookup,
    S: StudentSportLookup,
    C: CoachCandidateLookup,
    V: CoachReviewSummaryLookup,
    M: PublishedLessonMenuCountLookup,
    F: FavoriteCoachRepository,
    U: SuspendedCoachLookup,
{
    match account.role() {
        Role::Headquarters => {
            let Query(params) = headquarters_params
                .map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
            let acceptance = params
                .acceptance_status
                .as_deref()
                .map(coach_acceptance_status_from_contract)
                .transpose()?;
            let sport = params.sport.as_deref().map(admin_web_sport).transpose()?;
            let coaches = state
                .list_coaches
                .execute(acceptance, sport)
                .await
                .map_err(ApiError::internal)?;
            Ok(Json(CoachListBody {
                coaches: coaches.iter().map(coach_body).collect(),
            })
            .into_response())
        }
        Role::Student => {
            let Query(params) =
                params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
            let sport = params.sport.as_deref().map(student_app_sport).transpose()?;
            let command = ListCoachesForStudentCommand {
                student_id: account.account_id(),
                sport,
            };
            match state.list_coaches_for_student.execute(command).await {
                // 生徒自身のスポーツがゴルフ（student-app では登録できない）なら、golf-app の範囲なので空
                Ok(listings) => Ok(Json(CoachListingListBody {
                    coaches: listings
                        .iter()
                        .filter(|listing| listing.sport != Sport::Golf)
                        .map(coach_listing_body)
                        .collect(),
                })
                .into_response()),
                Err(ListCoachesForStudentError::StudentNotFound) => Ok(auth::unauthorized(
                    "トークンが示す生徒のアカウントが存在しない",
                )),
                Err(ListCoachesForStudentError::Unavailable(error)) => {
                    Err(ApiError::internal(error))
                }
            }
        }
        // require_role の層が先に 403 を返すので、ここには来ない
        Role::Coach => Err(ApiError::forbidden("コーチの役割ではこの操作はできない")),
    }
}

fn coach_body(coach: &ListedCoach) -> CoachBody {
    CoachBody {
        coach_id: account_id_uuid::format(coach.account.id()),
        display_name: coach.account.display_name().as_str().to_string(),
        acceptance_status: coach_acceptance_status_to_contract(coach.acceptance),
    }
}

fn coach_listing_body(listing: &CoachListing) -> CoachListingBody {
    CoachListingBody {
        coach_id: account_id_uuid::format(listing.coach_id),
        display_name: listing.display_name.as_str().to_string(),
        sport: sport_to_contract(listing.sport),
        review_count: listing.review_summary.review_count(),
        average_rating: listing.review_summary.average_rating(),
        published_lesson_menu_count: listing.published_lesson_menu_count,
        is_favorite: listing.is_favorite,
    }
}

/// コーチの一覧（`GET /coaches`）のルーター。組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）
/// 。
///
/// admin-web の `listCoaches`（本部）と student-app の `listCoachesForStudent`（生徒）が同じパスなので、1 つの
/// ハンドラーがトークンの役割で返す形を決める。コーチは 403。
pub fn coaches_router<R, A, S, C, V, M, F, U, T>(
    list_coaches: ListCoaches<R, A>,
    list_coaches_for_student: ListCoachesForStudent<S, C, V, M, F, U>,
    tokens: T,
) -> Router
where
    R: AccountRepository + 'static,
    A: CoachAcceptanceLookup + 'static,
    S: StudentSportLookup + 'static,
    C: CoachCandidateLookup + 'static,
    V: CoachReviewSummaryLookup + 'static,
    M: PublishedLessonMenuCountLookup + 'static,
    F: FavoriteCoachRepository + 'static,
    U: SuspendedCoachLookup + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/coaches",
            get(self::list_coaches::<R, A, S, C, V, M, F, U>),
        )
        .route_layer(require_role(&[Role::Headquarters, Role::Student]))
        .with_state(Arc::new(CoachesState {
            list_coaches,
            list_coaches_for_student,
        }))
        .layer(TokenVerification::layer(tokens))
}
