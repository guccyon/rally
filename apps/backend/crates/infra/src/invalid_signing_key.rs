//! 使えない署名の鍵（[`crate::SignedTokenService`] を作るときの失敗）。

use std::{error::Error, fmt};

use crate::{AccessTokenBackend, SignedTokenService};

/// アクセストークンの署名の鍵が短すぎる。`length` は渡された鍵のバイト数（鍵そのものは持たない）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidSigningKey {
    pub length: usize,
}

impl fmt::Display for InvalidSigningKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} は {} バイト以上（{} バイトしかない）",
            AccessTokenBackend::SIGNING_KEY_VAR,
            SignedTokenService::MIN_KEY_BYTES,
            self.length
        )
    }
}

impl Error for InvalidSigningKey {}
