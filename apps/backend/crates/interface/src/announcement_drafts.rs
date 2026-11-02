//! `/announcement-drafts`。admin-web の `POST /announcement-drafts`（`createAnnouncementDraft`。本部がお知らせの下書きを
//! 作る）と `GET /announcement-drafts`（`listAnnouncementDrafts`。下書きの一覧）と
//! `PUT /announcement-drafts/{announcement_draft_id}`（`updateAnnouncementDraft`。下書きを直す）と
//! `DELETE /announcement-drafts/{announcement_draft_id}`（`deleteAnnouncementDraft`。下書きを消す）と
//! `POST /announcement-drafts/{announcement_draft_id}/send`（`sendAnnouncementDraft`。下書きから送る）。
//! どれも本部だけが呼べる。
//!
//! 下書きから送ったお知らせの形は `announcements` の `Announcement` と同じ（変換は [`crate::announcements`] の 1 か所）。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::{TokenVerifier, account_id_uuid},
    notification::{
        AnnouncementDraftError, AnnouncementDraftRepository, AnnouncementRecipientLookup,
        AnnouncementRepository, AnnouncementWithReadCount, CreateAnnouncementDraft,
        DeleteAnnouncementDraft, ListAnnouncementDrafts, NotificationRepository,
        NotificationSettingsRepository, PushSender, SendAnnouncementDraft, UpdateAnnouncementDraft,
        announcement_draft_id_uuid,
    },
};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{post, put},
};
use domain::{
    catalog::Sport,
    identity::Role,
    notification::{AnnouncementDraft, AnnouncementDraftId, NotificationError},
};
use serde::{Deserialize, Serialize};

use crate::{
    announcements::announcement_body,
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    contract::{admin_web_sport, date_time_to_contract, sport_to_contract},
    error::ApiError,
    message::notification_error,
};

/// `/announcement-drafts` のハンドラーが使うユースケース。
pub struct AnnouncementDraftsState<D, A, L, N, S, P, C> {
    pub create_announcement_draft: CreateAnnouncementDraft<D, C>,
    pub list_announcement_drafts: ListAnnouncementDrafts<D>,
    pub update_announcement_draft: UpdateAnnouncementDraft<D, C>,
    pub delete_announcement_draft: DeleteAnnouncementDraft<D>,
    pub send_announcement_draft: SendAnnouncementDraft<D, A, L, N, S, P, C>,
}

/// ハンドラーが共有する [`AnnouncementDraftsState`]。
type Shared<D, A, L, N, S, P, C> = Arc<AnnouncementDraftsState<D, A, L, N, S, P, C>>;

/// admin-web の `AnnouncementDraftRequest`。`sport` は省くか null なら生徒全員。題・本文は空でもよい。
#[derive(Deserialize)]
pub struct AnnouncementDraftRequest {
    title: String,
    body: String,
    sport: Option<String>,
}

/// admin-web の `AnnouncementDraft`。
#[derive(Serialize)]
pub struct AnnouncementDraftBody {
    announcement_draft_id: String,
    created_by: String,
    title: String,
    body: String,
    sport: Option<&'static str>,
    created_at: String,
    updated_at: String,
}

/// admin-web の `AnnouncementDraftList`。
#[derive(Serialize)]
pub struct AnnouncementDraftListBody {
    announcement_drafts: Vec<AnnouncementDraftBody>,
}

/// 下書きの中身（題・本文・対象のスポーツ）。JSON でない・形が違う・スポーツの値でなければ 422。
fn draft_content(
    request: Result<Json<AnnouncementDraftRequest>, JsonRejection>,
) -> Result<(String, String, Option<Sport>), ApiError> {
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let sport = request.sport.as_deref().map(admin_web_sport).transpose()?;
    Ok((request.title, request.body, sport))
}

/// パスの下書きの ID。UUID でなければ 404（ない下書きと同じ）。
fn draft_id(text: &str) -> Result<AnnouncementDraftId, ApiError> {
    announcement_draft_id_uuid::parse(text).map_err(|_| draft_not_found())
}

fn draft_not_found() -> ApiError {
    ApiError::not_found("お知らせの下書きがない")
}

/// 下書きのユースケースの失敗。ない下書きは 404、対象の生徒がいなければ 404、ほかの不変条件の違反は 422。
fn draft_error(error: AnnouncementDraftError) -> ApiError {
    match error {
        AnnouncementDraftError::DraftNotFound => draft_not_found(),
        AnnouncementDraftError::Invalid(
            error @ NotificationError::AnnouncementWithoutRecipients,
        ) => ApiError::not_found(notification_error(&error)),
        AnnouncementDraftError::Invalid(error) => {
            ApiError::validation_failed(notification_error(&error))
        }
        AnnouncementDraftError::Unavailable(error) => ApiError::internal(error),
    }
}

/// `POST /announcement-drafts`（admin-web）。本部だけが呼べる（役割は `require_role` の層が確かめる）。作ったのはトークンの
/// 本部のアカウントとして残す。題・本文が長すぎる・スポーツが不正なら 422 で何も残さない。
pub async fn create_announcement_draft<D, A, L, N, S, P, C>(
    State(state): State<Shared<D, A, L, N, S, P, C>>,
    account: AuthenticatedAccount,
    request: Result<Json<AnnouncementDraftRequest>, JsonRejection>,
) -> Result<Response, ApiError>
where
    D: AnnouncementDraftRepository,
    A: AnnouncementRepository,
    L: AnnouncementRecipientLookup,
    N: NotificationRepository,
    S: NotificationSettingsRepository,
    P: PushSender,
    C: Clock,
{
    let (title, body, sport) = draft_content(request)?;
    let draft = state
        .create_announcement_draft
        .execute(account.account_id(), title, body, sport)
        .await
        .map_err(draft_error)?;
    Ok((StatusCode::CREATED, Json(draft_body(&draft)?)).into_response())
}

/// `GET /announcement-drafts`（admin-web）。本部だけが呼べる（役割は `require_role` の層が確かめる）。下書きを最後に
/// 直した時刻の新しい順で返す（どの本部のアカウントが作ったかで絞らない）。
pub async fn list_announcement_drafts<D, A, L, N, S, P, C>(
    State(state): State<Shared<D, A, L, N, S, P, C>>,
) -> Result<Json<AnnouncementDraftListBody>, ApiError>
where
    D: AnnouncementDraftRepository,
    A: AnnouncementRepository,
    L: AnnouncementRecipientLookup,
    N: NotificationRepository,
    S: NotificationSettingsRepository,
    P: PushSender,
    C: Clock,
{
    let drafts = state
        .list_announcement_drafts
        .execute()
        .await
        .map_err(ApiError::internal)?;
    draft_list_body(&drafts).map(Json)
}

/// `PUT /announcement-drafts/{announcement_draft_id}`（admin-web）。本部だけが呼べる（役割は `require_role` の層が
/// 確かめる）。題・本文・対象のスポーツをまとめて書き換え、直したあとの下書きを返す。ない・UUID でない下書きは 404、
/// 中身が不正なら 422 で、どちらも何も変えない。
pub async fn update_announcement_draft<D, A, L, N, S, P, C>(
    State(state): State<Shared<D, A, L, N, S, P, C>>,
    Path(announcement_draft_id): Path<String>,
    request: Result<Json<AnnouncementDraftRequest>, JsonRejection>,
) -> Result<Json<AnnouncementDraftBody>, ApiError>
where
    D: AnnouncementDraftRepository,
    A: AnnouncementRepository,
    L: AnnouncementRecipientLookup,
    N: NotificationRepository,
    S: NotificationSettingsRepository,
    P: PushSender,
    C: Clock,
{
    let draft_id = draft_id(&announcement_draft_id)?;
    let (title, body, sport) = draft_content(request)?;
    let draft = state
        .update_announcement_draft
        .execute(draft_id, title, body, sport)
        .await
        .map_err(draft_error)?;
    draft_body(&draft).map(Json)
}

/// `DELETE /announcement-drafts/{announcement_draft_id}`（admin-web）。本部だけが呼べる（役割は `require_role` の層が
/// 確かめる）。下書きを消し、残った下書きの一覧を返す。ない・UUID でない下書きは 404。
pub async fn delete_announcement_draft<D, A, L, N, S, P, C>(
    State(state): State<Shared<D, A, L, N, S, P, C>>,
    Path(announcement_draft_id): Path<String>,
) -> Result<Json<AnnouncementDraftListBody>, ApiError>
where
    D: AnnouncementDraftRepository,
    A: AnnouncementRepository,
    L: AnnouncementRecipientLookup,
    N: NotificationRepository,
    S: NotificationSettingsRepository,
    P: PushSender,
    C: Clock,
{
    let draft_id = draft_id(&announcement_draft_id)?;
    let drafts = state
        .delete_announcement_draft
        .execute(draft_id)
        .await
        .map_err(draft_error)?;
    draft_list_body(&drafts).map(Json)
}

/// `POST /announcement-drafts/{announcement_draft_id}/send`（admin-web）。本部だけが呼べる（役割は `require_role` の層が
/// 確かめる）。下書きの中身を `sendAnnouncement` と同じ規則で送り、送った記録を返す（送ったのはトークンの本部の
/// アカウント）。送れたら下書きは消える。ない・UUID でない下書きと、対象の生徒がいなければ 404、題・本文が空なら 422 で、
/// どちらも何も送らず下書きを残す。リクエストの本文は読まない。
pub async fn send_announcement_draft<D, A, L, N, S, P, C>(
    State(state): State<Shared<D, A, L, N, S, P, C>>,
    account: AuthenticatedAccount,
    Path(announcement_draft_id): Path<String>,
) -> Result<Response, ApiError>
where
    D: AnnouncementDraftRepository,
    A: AnnouncementRepository,
    L: AnnouncementRecipientLookup,
    N: NotificationRepository,
    S: NotificationSettingsRepository,
    P: PushSender,
    C: Clock,
{
    let draft_id = draft_id(&announcement_draft_id)?;
    let announcement = state
        .send_announcement_draft
        .execute(draft_id, account.account_id())
        .await
        .map_err(draft_error)?;
    Ok((
        StatusCode::CREATED,
        Json(announcement_body(&AnnouncementWithReadCount::just_sent(
            announcement,
        ))?),
    )
        .into_response())
}

fn draft_body(draft: &AnnouncementDraft) -> Result<AnnouncementDraftBody, ApiError> {
    Ok(AnnouncementDraftBody {
        announcement_draft_id: announcement_draft_id_uuid::format(draft.id()),
        created_by: account_id_uuid::format(draft.created_by()),
        title: draft.title().to_string(),
        body: draft.body().to_string(),
        sport: draft.sport().map(sport_to_contract),
        created_at: date_time_to_contract(draft.created_at())?,
        updated_at: date_time_to_contract(draft.updated_at())?,
    })
}

fn draft_list_body(drafts: &[AnnouncementDraft]) -> Result<AnnouncementDraftListBody, ApiError> {
    Ok(AnnouncementDraftListBody {
        announcement_drafts: drafts.iter().map(draft_body).collect::<Result<_, _>>()?,
    })
}

/// 本部からのお知らせの下書き（admin-web の `/announcement-drafts` の 5 つの操作）のルーター。組み立て用のクレートは
/// 他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// どれも本部だけ（コーチ・生徒は 403）。下書きは生徒に届かず、下書きから送ったものは
/// [`crate::announcements_router`] の一覧（`listAnnouncements`）に出る。
#[allow(clippy::too_many_arguments)]
pub fn announcement_drafts_router<D, A, L, N, S, P, C, T>(
    create_announcement_draft: CreateAnnouncementDraft<D, C>,
    list_announcement_drafts: ListAnnouncementDrafts<D>,
    update_announcement_draft: UpdateAnnouncementDraft<D, C>,
    delete_announcement_draft: DeleteAnnouncementDraft<D>,
    send_announcement_draft: SendAnnouncementDraft<D, A, L, N, S, P, C>,
    tokens: T,
) -> Router
where
    D: AnnouncementDraftRepository + 'static,
    A: AnnouncementRepository + 'static,
    L: AnnouncementRecipientLookup + 'static,
    N: NotificationRepository + 'static,
    S: NotificationSettingsRepository + 'static,
    P: PushSender + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/announcement-drafts",
            post(self::create_announcement_draft::<D, A, L, N, S, P, C>)
                .get(self::list_announcement_drafts::<D, A, L, N, S, P, C>),
        )
        .route(
            "/announcement-drafts/{announcement_draft_id}",
            put(self::update_announcement_draft::<D, A, L, N, S, P, C>)
                .delete(self::delete_announcement_draft::<D, A, L, N, S, P, C>),
        )
        .route(
            "/announcement-drafts/{announcement_draft_id}/send",
            post(self::send_announcement_draft::<D, A, L, N, S, P, C>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(AnnouncementDraftsState {
            create_announcement_draft,
            list_announcement_drafts,
            update_announcement_draft,
            delete_announcement_draft,
            send_announcement_draft,
        }))
        .layer(TokenVerification::layer(tokens))
}
