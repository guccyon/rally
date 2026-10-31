//! `POST /coaches/me/slots/{slot_id}/withdraw`。admin-web の `withdrawMySlot`（コーチが公開した枠の取り下げ）。
//! コーチだけが呼べる。

use std::sync::Arc;

use application::{
    analytics::AnalyticsSink,
    clock::Clock,
    identity::TokenVerifier,
    scheduling::{SlotRepository, WithdrawSlot, WithdrawSlotCommand, slot_id_uuid},
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::post,
};
use domain::identity::Role;

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    error::ApiError,
    scheduling_error::coach_slot_not_found,
    slots::{AdminWebSlotBody, admin_web_slot_body},
};

/// `/coaches/me/slots/{slot_id}/withdraw` のハンドラーが使うユースケース。
pub struct SlotWithdrawalsState<S, A, C> {
    pub withdraw_slot: WithdrawSlot<S, A, C>,
}

/// `POST /coaches/me/slots/{slot_id}/withdraw`。コーチだけが呼べ（役割は `require_role` の層が確かめる）、取り下げられる
/// のはトークンのコーチ自身の枠だけ。取り下げた枠（`closed`）を返す。応答の `Slot` には `lesson_menu` を載せない
/// （`publishSlot` と同じ）。失敗の応答は [`crate::scheduling_error`] が決める。
pub async fn withdraw_my_slot<S, A, C>(
    State(state): State<Arc<SlotWithdrawalsState<S, A, C>>>,
    account: AuthenticatedAccount,
    Path(slot_id): Path<String>,
) -> Result<Json<AdminWebSlotBody>, ApiError>
where
    S: SlotRepository,
    A: AnalyticsSink,
    C: Clock,
{
    // 契約の withdrawMySlot に 422 はないので、UUID でない ID はない枠として扱う
    let slot_id = slot_id_uuid::parse(&slot_id).map_err(|_| coach_slot_not_found())?;

    let slot = state
        .withdraw_slot
        .execute(WithdrawSlotCommand {
            coach_id: account.account_id(),
            slot_id,
        })
        .await
        .map_err(ApiError::from)?;
    Ok(Json(admin_web_slot_body(&slot, None)?))
}

/// コーチが公開した枠の取り下げ（admin-web の `POST /coaches/me/slots/{slot_id}/withdraw`）のルーター。
/// 組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// `POST /coaches/me/slots/{slot_id}/withdraw` はコーチだけで、取り下げられるのはトークンのコーチ自身の枠だけ。
pub fn slot_withdrawal_router<S, A, C, T>(withdraw_slot: WithdrawSlot<S, A, C>, tokens: T) -> Router
where
    S: SlotRepository + 'static,
    A: AnalyticsSink + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/coaches/me/slots/{slot_id}/withdraw",
            post(self::withdraw_my_slot::<S, A, C>),
        )
        .route_layer(require_role(&[Role::Coach]))
        .with_state(Arc::new(SlotWithdrawalsState { withdraw_slot }))
        .layer(TokenVerification::layer(tokens))
}
