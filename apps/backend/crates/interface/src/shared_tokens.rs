//! 1 つのアクセストークンのポートを、機能ごとのルーターで分けて持つための包み（[`crate::router_with`] が使う）。

use std::sync::Arc;

use application::identity::{AccessToken, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier};

/// `T` を共有して複製できるようにしたアクセストークンのポート。発行・検証は `T` にそのまま任せる。
pub(crate) struct SharedTokens<T>(Arc<T>);

impl<T> SharedTokens<T> {
    pub(crate) fn new(tokens: T) -> Self {
        Self(Arc::new(tokens))
    }
}

impl<T> Clone for SharedTokens<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: TokenIssuer> TokenIssuer for SharedTokens<T> {
    fn issue(&self, claims: TokenClaims) -> AccessToken {
        self.0.issue(claims)
    }
}

impl<T: TokenVerifier> TokenVerifier for SharedTokens<T> {
    fn verify(&self, token: &str) -> Result<TokenClaims, InvalidToken> {
        self.0.verify(token)
    }
}
