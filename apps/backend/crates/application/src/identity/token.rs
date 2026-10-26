//! アクセストークンの発行・検証のポート。
//!
//! トークンはアカウント ID と役割を載せた不透明な文字列で、クライアントは中身を読まない
//! （`Authorization: Bearer <access_token>`。OpenAPI の `bearerAuth`）。Gen1 の実装は
//! infra のモック（署名なし）で、本番では署名付きの実装に差し替える。パスワード・外部 IdP・
//! 失効はこのポートの範囲外。

use domain::identity::{AccountId, Role};

/// トークンに載せる内容。検証に通れば発行時と同じ値が返る。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenClaims {
    pub account_id: AccountId,
    pub role: Role,
}

/// 不透明なアクセストークン。中身の形は実装が決める。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessToken(String);

impl AccessToken {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

/// トークンが無効（形が不正、または発行したものでない）。理由はクライアントに区別させない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidToken;

/// アクセストークンを発行するポート。
pub trait TokenIssuer: Send + Sync {
    fn issue(&self, claims: TokenClaims) -> AccessToken;
}

/// アクセストークンを検証し、載っている内容を返すポート。
pub trait TokenVerifier: Send + Sync {
    fn verify(&self, token: &str) -> Result<TokenClaims, InvalidToken>;
}
