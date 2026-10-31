//! `/favorite-coaches`。student-app の `GET /favorite-coaches`（`listFavoriteCoaches`。お気に入りのコーチの一覧）と
//! `PUT`・`DELETE /favorite-coaches/{coach_id}`（`addFavoriteCoach`・`removeFavoriteCoach`。お気に入りに入れる・外す）。
//! 生徒だけが呼べ、扱うのはトークンの生徒自身のお気に入りだけ。入れる・外すは、変えたあとの一覧を返す。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::{AccountRepository, account_id_uuid},
    matching::{
        AddFavoriteCoach, AddFavoriteCoachCommand, AddFavoriteCoachError, CoachReviewSummaryLookup,
        FavoriteCoachListing, FavoriteCoachRepository, ListFavoriteCoaches, RemoveFavoriteCoach,
    },
};
use axum::{
    Json,
    extract::{Path, State},
};
use domain::{identity::AccountId, matching::MatchingError};
use serde::Serialize;

use crate::{
    auth::AuthenticatedAccount,
    contract::{date_time_to_contract, student_app_sports, student_app_sports_to_contract},
    error::ApiError,
};

/// `/favorite-coaches` のハンドラーが使うユースケース。
pub struct FavoriteCoachesState<F, R, C, V> {
    pub add_favorite_coach: AddFavoriteCoach<F, R, C>,
    pub remove_favorite_coach: RemoveFavoriteCoach<F>,
    pub list_favorite_coaches: ListFavoriteCoaches<F, R, V>,
}

/// ハンドラーが共有する [`FavoriteCoachesState`]。
type Shared<F, R, C, V> = Arc<FavoriteCoachesState<F, R, C, V>>;

/// student-app の `FavoriteCoachList`。
#[derive(Serialize)]
pub struct FavoriteCoachListBody {
    favorite_coaches: Vec<FavoriteCoachBody>,
}

/// student-app の `FavoriteCoach`。
#[derive(Serialize)]
pub struct FavoriteCoachBody {
    coach_id: String,
    display_name: String,
    sports: Vec<&'static str>,
    review_count: u32,
    average_rating: Option<f64>,
    favorited_at: String,
}

/// `GET /favorite-coaches`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。
pub async fn list_favorite_coaches<F, R, C, V>(
    State(state): State<Shared<F, R, C, V>>,
    account: AuthenticatedAccount,
) -> Result<Json<FavoriteCoachListBody>, ApiError>
where
    F: FavoriteCoachRepository,
    R: AccountRepository,
    C: Clock,
    V: CoachReviewSummaryLookup,
{
    list(&state, account.account_id()).await
}

/// `PUT /favorite-coaches/{coach_id}`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。コーチでない ID・
/// ゴルフだけのコーチ（`getCoachProfile` が 404 のコーチ）・UUID でない ID は 404。
pub async fn add_favorite_coach<F, R, C, V>(
    State(state): State<Shared<F, R, C, V>>,
    account: AuthenticatedAccount,
    Path(coach_id): Path<String>,
) -> Result<Json<FavoriteCoachListBody>, ApiError>
where
    F: FavoriteCoachRepository,
    R: AccountRepository,
    C: Clock,
    V: CoachReviewSummaryLookup,
{
    let coach_id = parse_coach_id(&coach_id)?;
    let command = AddFavoriteCoachCommand {
        student_id: account.account_id(),
        coach_id,
        sports: student_app_sports(),
    };
    match state.add_favorite_coach.execute(command).await {
        Ok(()) => {}
        Err(AddFavoriteCoachError::CoachNotFound)
        | Err(AddFavoriteCoachError::Rejected(MatchingError::NotACoach)) => {
            return Err(coach_not_found());
        }
        // トークンは生徒の役割だが、そのアカウントがない・生徒でない
        Err(AddFavoriteCoachError::StudentNotFound)
        | Err(AddFavoriteCoachError::Rejected(MatchingError::NotAStudent)) => {
            return Err(ApiError::unauthorized(
                "トークンが示す生徒のアカウントが存在しない",
            ));
        }
        Err(AddFavoriteCoachError::Unavailable(error)) => return Err(ApiError::internal(error)),
    }
    list(&state, account.account_id()).await
}

/// `DELETE /favorite-coaches/{coach_id}`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。入っていない
/// コーチを外しても 200。UUID でない ID は 404。
pub async fn remove_favorite_coach<F, R, C, V>(
    State(state): State<Shared<F, R, C, V>>,
    account: AuthenticatedAccount,
    Path(coach_id): Path<String>,
) -> Result<Json<FavoriteCoachListBody>, ApiError>
where
    F: FavoriteCoachRepository,
    R: AccountRepository,
    C: Clock,
    V: CoachReviewSummaryLookup,
{
    let coach_id = parse_coach_id(&coach_id)?;
    state
        .remove_favorite_coach
        .execute(account.account_id(), coach_id)
        .await
        .map_err(ApiError::internal)?;
    list(&state, account.account_id()).await
}

/// 契約の `coach_id` では、UUID でない ID はないものとして 404。
fn parse_coach_id(coach_id: &str) -> Result<AccountId, ApiError> {
    account_id_uuid::parse(coach_id).map_err(|_| coach_not_found())
}

fn coach_not_found() -> ApiError {
    ApiError::not_found("コーチがない")
}

/// 生徒 `student_id` のお気に入りの一覧。ゴルフだけのコーチは student-app の範囲でないので載せない。
async fn list<F, R, C, V>(
    state: &FavoriteCoachesState<F, R, C, V>,
    student_id: AccountId,
) -> Result<Json<FavoriteCoachListBody>, ApiError>
where
    F: FavoriteCoachRepository,
    R: AccountRepository,
    C: Clock,
    V: CoachReviewSummaryLookup,
{
    let listings = state
        .list_favorite_coaches
        .execute(student_id)
        .await
        .map_err(ApiError::internal)?;
    let mut favorite_coaches = Vec::with_capacity(listings.len());
    for listing in &listings {
        if let Some(body) = favorite_coach_body(listing)? {
            favorite_coaches.push(body);
        }
    }
    Ok(Json(FavoriteCoachListBody { favorite_coaches }))
}

fn favorite_coach_body(
    listing: &FavoriteCoachListing,
) -> Result<Option<FavoriteCoachBody>, ApiError> {
    let sports = student_app_sports_to_contract(&listing.sports);
    if sports.is_empty() {
        return Ok(None);
    }
    Ok(Some(FavoriteCoachBody {
        coach_id: account_id_uuid::format(listing.coach_id),
        display_name: listing.display_name.as_str().to_string(),
        sports,
        review_count: listing.review_summary.review_count(),
        average_rating: listing.review_summary.average_rating(),
        favorited_at: date_time_to_contract(listing.favorited_at)?,
    }))
}
