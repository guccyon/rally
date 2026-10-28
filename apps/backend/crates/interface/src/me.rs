//! `/me`。admin-web の `GET /me`（`getMyAccount`）。役割では絞らず、有効なトークンなら誰でも呼べる。

use std::sync::Arc;

use application::identity::{AccountRepository, GetAccount, account_id_uuid};
use axum::{
    Json,
    extract::State,
    response::{IntoResponse, Response},
};
use domain::identity::Account;
use serde::Serialize;

use crate::{
    auth::{self, AuthenticatedAccount},
    contract::role_to_contract,
    error::ApiError,
};

/// `/me` のハンドラーが使うユースケース。
pub struct MeState<R> {
    pub get_account: GetAccount<R>,
}

/// admin-web の `Account`。
#[derive(Serialize)]
pub struct AccountBody {
    account_id: String,
    role: &'static str,
    display_name: String,
}

/// `GET /me`。照会するのはトークンのアカウントで、存在しないか役割がトークンと合わなければ
/// 無効なトークンとして 401。
pub async fn get_my_account<R>(
    State(state): State<Arc<MeState<R>>>,
    account: AuthenticatedAccount,
) -> Response
where
    R: AccountRepository,
{
    match state.get_account.execute(account.account_id()).await {
        Ok(Some(found)) if found.role() == account.role() => {
            Json(account_body(&found)).into_response()
        }
        Ok(_) => auth::unauthorized("トークンが示すアカウントが存在しないか、役割が合わない"),
        Err(error) => ApiError::internal(error).into_response(),
    }
}

fn account_body(account: &Account) -> AccountBody {
    AccountBody {
        account_id: account_id_uuid::format(account.id()),
        role: role_to_contract(account.role()),
        display_name: account.display_name().as_str().to_string(),
    }
}
