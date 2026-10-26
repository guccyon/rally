//! 認証（`Authorization: Bearer <access_token>`。OpenAPI の `bearerAuth`）と役割による認可。
//!
//! `POST /accounts` と `GET /healthz` 以外のエンドポイントは認証必須で、ハンドラーの引数に
//! [`AuthenticatedAccount`] を取る（トークンがない・無効なら 401）。役割で絞るエンドポイントは
//! [`require_role`] を `route_layer` で重ねる（役割が合わなければ 403）。admin-web の本部と
//! コーチは同じアプリに役割で同居する（ADR 0002 Q3）。

use std::{convert::Infallible, future::Future, sync::Arc};

use application::identity::{TokenClaims, TokenVerifier};
use axum::{
    Extension,
    extract::{FromRequestParts, Request},
    http::{HeaderValue, header, request::Parts},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::Route,
};
use domain::identity::{AccountId, Role};
use tower::{Layer, Service};

use crate::error::ApiError;

/// 検証に使う [`TokenVerifier`]。[`TokenVerification::layer`] でリクエストに載せ、
/// [`AuthenticatedAccount`] がそこから読む。
#[derive(Clone)]
pub struct TokenVerification(Arc<dyn TokenVerifier>);

impl TokenVerification {
    /// ルーターに重ねる層。これより内側のルートで [`AuthenticatedAccount`] が使える。
    pub fn layer(verifier: Arc<dyn TokenVerifier>) -> Extension<Self> {
        Extension(Self(verifier))
    }
}

/// 有効なトークンを示したアカウント。抽出できなければ 401 `unauthorized`。
///
/// 作れるのは抽出子（[`FromRequestParts`]）だけで、トークンを検証せずに外から組み立てることは
/// できない。中身はアクセサで読む。
///
/// ```compile_fail
/// use domain::identity::{AccountId, Role};
/// use interface::AuthenticatedAccount;
///
/// let _ = AuthenticatedAccount {
///     account_id: AccountId::from_u128(42),
///     role: Role::Headquarters,
/// };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthenticatedAccount {
    account_id: AccountId,
    role: Role,
}

impl AuthenticatedAccount {
    /// トークンが示したアカウントの ID。
    pub fn account_id(&self) -> AccountId {
        self.account_id
    }

    /// トークンが示した役割。
    pub fn role(&self) -> Role {
        self.role
    }

    /// 役割が `allowed` のどれかでなければ 403 `forbidden`。
    pub fn require_role(&self, allowed: &[Role]) -> Result<(), ApiError> {
        if allowed.contains(&self.role) {
            Ok(())
        } else {
            Err(ApiError::forbidden(format!(
                "{}の役割ではこの操作はできない",
                crate::message::role_label(self.role)
            )))
        }
    }
}

impl<S: Send + Sync> FromRequestParts<S> for AuthenticatedAccount {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Some(TokenVerification(verifier)) = parts.extensions.get::<TokenVerification>() else {
            return Err(
                ApiError::internal("TokenVerification の層が組み立てられていない").into_response(),
            );
        };
        let token = bearer_token(parts).ok_or_else(|| unauthorized("Bearer のトークンがない"))?;
        let TokenClaims { account_id, role } = verifier
            .verify(token)
            .map_err(|_| unauthorized("トークンが無効"))?;
        Ok(Self { account_id, role })
    }
}

/// `Authorization: Bearer <token>` のトークン（スキーム名は大文字小文字を問わない。RFC 6750）。
fn bearer_token(parts: &Parts) -> Option<&str> {
    let value = parts.headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

/// 401 と `WWW-Authenticate: Bearer`（RFC 6750）。
fn unauthorized(message: &str) -> Response {
    let mut response = ApiError::unauthorized(message).into_response();
    response
        .headers_mut()
        .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    response
}

/// 役割が `allowed` のどれかのアカウントだけを通す層。`route_layer` で重ねる。
///
/// トークンがない・無効なら 401、役割が合わなければ 403 で、内側のハンドラーは呼ばれない。
///
/// ```text
/// Router::new()
///     .route("/slots", post(publish_slot))
///     .route_layer(require_role(&[Role::Coach]))
/// ```
pub fn require_role(
    allowed: &'static [Role],
) -> impl Layer<
    Route,
    Service = impl Service<
        Request,
        Response = Response,
        Error = Infallible,
        Future = impl Future<Output = Result<Response, Infallible>> + Send + 'static,
    > + Clone
              + Send
              + Sync
              + 'static,
> + Clone
+ Send
+ Sync
+ 'static {
    middleware::from_fn(
        move |account: AuthenticatedAccount, request: Request, next: Next| async move {
            match account.require_role(allowed) {
                Ok(()) => next.run(request).await,
                Err(error) => error.into_response(),
            }
        },
    )
}
