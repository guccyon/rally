//! アクセストークンの実装を環境変数で選ぶ（server の起動時と、staging でトークンを作る `access-token` が使う。Issue #1593・#1785）。

use application::identity::{AccessToken, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier};

use crate::{AccessTokenConfigError, MockTokenService, SignedTokenService};

/// アクセストークンの発行・検証。`ACCESS_TOKEN_SIGNING_KEY` と `ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK` で選ぶ。本部・コーチ・生徒の
/// どのトークンも同じものが発行・検証する。
///
/// | `ACCESS_TOKEN_SIGNING_KEY` | `ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK` | 実装 |
/// |---|---|---|
/// | 32 バイト以上の値 | 何でも（読まない） | 署名付き（[`SignedTokenService`]。手で作った `mock.` のトークンや改ざんしたトークンは通らない） |
/// | なし・空 | `1` | 署名なしのモック（[`MockTokenService`]。`mock.<役割>.<アカウント ID>`。ローカルの起動とテスト用） |
/// | なし・空 | なし・空 | 選べない（[`AccessTokenConfigError::MissingSigningKey`]。起動しない） |
///
/// 鍵がなければ既定で失敗にする（fail-closed。ADR 0016）。鍵は秘密情報で、リポジトリに置かない。本番と staging への配り方は
/// 人が用意する（apps/backend の README の「認証と認可」）。
#[derive(Debug, Clone)]
pub enum AccessTokenBackend {
    Mock(MockTokenService),
    Signed(SignedTokenService),
}

impl AccessTokenBackend {
    pub const SIGNING_KEY_VAR: &'static str = "ACCESS_TOKEN_SIGNING_KEY";
    /// 署名なしのモックを明示して選ぶ環境変数（値は `1`）。鍵があれば読まない。`deploy/compose.yaml` は渡さない。
    pub const UNSIGNED_MOCK_OPT_IN_VAR: &'static str = "ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK";

    /// プロセスの環境変数から選ぶ。
    pub fn from_env() -> Result<Self, AccessTokenConfigError> {
        Self::from_vars(|name| std::env::var(name).ok())
    }

    /// `var`（環境変数の名前から値）で選ぶ。鍵があれば署名付き（短ければ失敗）。鍵がなければ opt-in が `1` のときだけ
    /// モックにし、opt-in がない・`1` でなければ失敗にする（黙ってモックに倒さない）。
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Result<Self, AccessTokenConfigError> {
        if let Some(key) = var(Self::SIGNING_KEY_VAR).filter(|key| !key.is_empty()) {
            return Ok(Self::Signed(SignedTokenService::new(key.as_bytes())?));
        }
        match var(Self::UNSIGNED_MOCK_OPT_IN_VAR).as_deref() {
            Some("1") => Ok(Self::Mock(MockTokenService)),
            None | Some("") => Err(AccessTokenConfigError::MissingSigningKey),
            Some(other) => Err(AccessTokenConfigError::InvalidMockOptIn {
                value: other.to_string(),
            }),
        }
    }

    /// 署名付きのトークンか。
    pub fn is_signed(&self) -> bool {
        matches!(self, Self::Signed(_))
    }
}

/// 署名なしのモック。テストの組み立て（`server::router` など）だけが使い、`main` と `access-token` は [`Self::from_env`] で選ぶ。
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
