//! アクセストークンの実装を選べない設定（[`crate::AccessTokenBackend`] を選ぶときの失敗）。

use std::{error::Error, fmt};

use crate::{AccessTokenBackend, InvalidSigningKey};

/// `ACCESS_TOKEN_SIGNING_KEY` と `ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK` から実装を選べない。どれも起動を止める
/// （黙って署名なしのモックに倒さない。Issue #1785）。鍵の値は持たない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessTokenConfigError {
    /// 鍵がなく（ないか空）、署名なしのモックの opt-in もない。
    MissingSigningKey,
    /// 鍵が短すぎる。
    InvalidSigningKey(InvalidSigningKey),
    /// opt-in の値が `1` でない。`value` は指定された生の値（秘密ではない）。
    InvalidMockOptIn { value: String },
}

impl fmt::Display for AccessTokenConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSigningKey => write!(
                f,
                "{} がない。アクセストークンの署名の鍵（32 バイト以上）を渡す。署名なしのモックのトークンで動かすのは\
                 ローカルの起動とテストだけで、そのときは {}=1 を渡す（apps/backend の README の「認証と認可」）",
                AccessTokenBackend::SIGNING_KEY_VAR,
                AccessTokenBackend::UNSIGNED_MOCK_OPT_IN_VAR
            ),
            Self::InvalidSigningKey(error) => error.fmt(f),
            Self::InvalidMockOptIn { value } => write!(
                f,
                "{} は 1 だけ（{value:?} は知らない）",
                AccessTokenBackend::UNSIGNED_MOCK_OPT_IN_VAR
            ),
        }
    }
}

impl Error for AccessTokenConfigError {}

impl From<InvalidSigningKey> for AccessTokenConfigError {
    fn from(error: InvalidSigningKey) -> Self {
        Self::InvalidSigningKey(error)
    }
}
