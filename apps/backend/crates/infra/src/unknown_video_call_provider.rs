//! 知らない通話基盤の指定（[`crate::VideoCallBackend`] を選ぶときの失敗）。

use std::{error::Error, fmt};

use crate::VideoCallBackend;

/// `VIDEO_CALL_PROVIDER` の値が `twilio` でも `alt` でもない。`value` は指定された生の値。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownVideoCallProvider {
    pub value: String,
}

impl fmt::Display for UnknownVideoCallProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} は twilio か alt（{:?} は知らない）",
            VideoCallBackend::PROVIDER_VAR,
            self.value
        )
    }
}

impl Error for UnknownVideoCallProvider {}
