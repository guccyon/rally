//! アクセストークンの実装を環境変数で選ぶ（server の起動時と、staging でトークンを作る `access-token` が使う。Issue #1593）。

use application::identity::{AccessToken, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier};

use crate::{InvalidSigningKey, MockTokenService, SignedTokenService};

/// アクセストークンの発行・検証。`ACCESS_TOKEN_SIGNING_KEY` があるかで選ぶ。本部・コーチ・生徒のどのトークンも同じものが
/// 発行・検証する。
///
/// | `ACCESS_TOKEN_SIGNING_KEY` | 実装 |
/// |---|---|
/// | なし・空 | 署名なしのモック（[`MockTokenService`]。`mock.<役割>.<アカウント ID>`。ローカルの起動とテスト用） |
/// | 32 バイト以上の値 | 署名付き（[`SignedTokenService`]。手で作った `mock.` のトークンや改ざんしたトークンは通らない） |
///
/// 鍵は秘密情報で、リポジトリに置かない。本番と staging への配り方は人が用意する（apps/backend の README の「認証」）。
#[derive(Debug, Clone)]
pub enum AccessTokenBackend {
    Mock(MockTokenService),
    Signed(SignedTokenService),
}

impl AccessTokenBackend {
    pub const SIGNING_KEY_VAR: &'static str = "ACCESS_TOKEN_SIGNING_KEY";

    /// プロセスの環境変数から選ぶ。
    pub fn from_env() -> Result<Self, InvalidSigningKey> {
        Self::from_vars(|name| std::env::var(name).ok())
    }

    /// `var`（環境変数の名前から値）で選ぶ。鍵が短ければ [`InvalidSigningKey`] で失敗にする（黙ってモックに倒さない）。
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Result<Self, InvalidSigningKey> {
        match var(Self::SIGNING_KEY_VAR).filter(|key| !key.is_empty()) {
            None => Ok(Self::Mock(MockTokenService)),
            Some(key) => SignedTokenService::new(key.as_bytes()).map(Self::Signed),
        }
    }

    /// 署名付きのトークンか。
    pub fn is_signed(&self) -> bool {
        matches!(self, Self::Signed(_))
    }
}

impl Default for AccessTokenBackend {
    fn default() -> Self {
        Self::Mock(MockTokenService)
    }
}

impl TokenIssuer for AccessTokenBackend {
    fn issue(&self, claims: TokenClaims) -> AccessToken {
        match self {
            Self::Mock(tokens) => tokens.issue(claims),
            Self::Signed(tokens) => tokens.issue(claims),
        }
    }
}

impl TokenVerifier for AccessTokenBackend {
    fn verify(&self, token: &str) -> Result<TokenClaims, InvalidToken> {
        match self {
            Self::Mock(tokens) => tokens.verify(token),
            Self::Signed(tokens) => tokens.verify(token),
        }
    }
}
