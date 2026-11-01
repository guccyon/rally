//! `/lessons/completions/me`。student-app の `GET /lessons/completions/me`（`listMyLessonCompletions`。生徒が受けた
//! レッスンの一覧）。生徒だけが呼べる。

use std::sync::Arc;

use application::{
    async_lesson::VideoRepository,
    catalog::lesson_menu_id_uuid,
    identity::{TokenVerifier, account_id_uuid},
    lesson::{
        LessonCompletionCoachDisplayNameLookup, LessonCompletionRepository,
        LessonCompletionReviewLookup, ListMyLessonCompletions, lesson_completion_id_uuid,
    },
    live_lesson::{BookingLookup, LiveLessonSessionRepository},
};
use axum::{Json, Router, extract::State, routing::get};
use domain::{identity::Role, lesson::LessonCompletionMenu};
use serde::Serialize;

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    contract::{date_time_to_contract, lesson_type_to_contract},
    error::ApiError,
    lessons::source_ids,
};

/// `/lessons/completions/me` のハンドラーが使うユースケース。
pub struct MyLessonCompletionsState<R, L, B, W, D, V> {
    pub list_my_lesson_completions: ListMyLessonCompletions<R, L, B, W, D, V>,
}

/// ハンドラーが共有する [`MyLessonCompletionsState`]。
type Shared<R, L, B, W, D, V> = Arc<MyLessonCompletionsState<R, L, B, W, D, V>>;

/// student-app の `MyLessonCompletionList`。
#[derive(Serialize)]
pub struct MyLessonCompletionListBody {
    lesson_completions: Vec<MyLessonCompletionBody>,
}

/// student-app の `MyLessonCompletion`。コーチの表示名は引けなければ載せない。種類に合わない元の ID と、元にない
/// レッスンメニューは null。
#[derive(Serialize)]
pub struct MyLessonCompletionBody {
    lesson_id: String,
    coach_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    coach_display_name: Option<String>,
    lesson_type: &'static str,
    session_id: Option<String>,
    video_id: Option<String>,
    completed_at: String,
    lesson_menu: Option<LessonCompletionMenuBody>,
    is_reviewed: bool,
}

/// student-app の `LessonCompletionMenu`（完了したレッスンのレッスンメニューの ID・名前）。
#[derive(Serialize)]
pub struct LessonCompletionMenuBody {
    lesson_menu_id: String,
    name: String,
}

/// `GET /lessons/completions/me`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。照会するのはトークンの
/// アカウントが生徒の完了したレッスン。
pub async fn list_my_lesson_completions<R, L, B, W, D, V>(
    State(state): State<Shared<R, L, B, W, D, V>>,
    account: AuthenticatedAccount,
) -> Result<Json<MyLessonCompletionListBody>, ApiError>
where
    R: LessonCompletionRepository,
    L: LiveLessonSessionRepository,
    B: BookingLookup,
    W: VideoRepository,
    D: LessonCompletionCoachDisplayNameLookup,
    V: LessonCompletionReviewLookup,
{
    let list = state
        .list_my_lesson_completions
        .execute(account.account_id())
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(MyLessonCompletionListBody {
        lesson_completions: list
            .completions
            .iter()
            .map(|item| {
                let completion = &item.completion;
                let (session_id, video_id) = source_ids(completion.source());
                Ok(MyLessonCompletionBody {
                    lesson_id: lesson_completion_id_uuid::format(completion.id()),
                    coach_id: account_id_uuid::format(completion.coach_id()),
                    coach_display_name: list
                        .coaches
                        .get(&completion.coach_id())
                        .map(|name| name.as_str().to_string()),
                    lesson_type: lesson_type_to_contract(completion.lesson_type()),
                    session_id,
                    video_id,
                    completed_at: date_time_to_contract(completion.completed_at())?,
                    lesson_menu: item.lesson_menu.as_ref().map(lesson_completion_menu_body),
                    is_reviewed: item.reviewed,
                })
            })
            .collect::<Result<_, ApiError>>()?,
    }))
}

fn lesson_completion_menu_body(menu: &LessonCompletionMenu) -> LessonCompletionMenuBody {
    LessonCompletionMenuBody {
        lesson_menu_id: lesson_menu_id_uuid::format(menu.id()),
        name: menu.name().as_str().to_string(),
    }
}

/// 生徒が受けたレッスンの一覧（student-app の `GET /lessons/completions/me`）のルーター。
/// 組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// `GET /lessons/completions/me` は生徒だけ（コーチ・本部は 403）。同じ接頭辞の `POST /lessons/completions`（完了記録）
/// は [`crate::lessons_router`] で、役割で絞らない。
pub fn my_lesson_completions_router<R, L, B, W, D, V, T>(
    list_my_lesson_completions: ListMyLessonCompletions<R, L, B, W, D, V>,
    tokens: T,
) -> Router
where
    R: LessonCompletionRepository + 'static,
    L: LiveLessonSessionRepository + 'static,
    B: BookingLookup + 'static,
    W: VideoRepository + 'static,
    D: LessonCompletionCoachDisplayNameLookup + 'static,
    V: LessonCompletionReviewLookup + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/lessons/completions/me",
            get(self::list_my_lesson_completions::<R, L, B, W, D, V>),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(MyLessonCompletionsState {
            list_my_lesson_completions,
        }))
        .layer(TokenVerification::layer(tokens))
}
