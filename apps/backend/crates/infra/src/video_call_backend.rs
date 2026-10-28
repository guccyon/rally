//! 通話基盤を環境変数で選ぶ（server の起動時に使う。変更シナリオ cs-007）。

use application::{Unavailable, live_lesson::VideoCallProvider};
use domain::{
    identity::AccountId,
    live_lesson::{ConnectionQuality, SessionId},
};

use crate::{MockAltVideoCallProvider, MockVideoCallProvider, UnknownVideoCallProvider};

/// 通話基盤。`VIDEO_CALL_PROVIDER` で選ぶ。どちらもモックで、ポート [`VideoCallProvider`] の形は同じ。
///
/// | `VIDEO_CALL_PROVIDER` | 通話基盤 |
/// |---|---|
/// | なし・`twilio` | Twilio Video 相当（[`MockVideoCallProvider`]） |
/// | `alt` | 生の値の単位が違う別の基盤（[`MockAltVideoCallProvider`]。換算はアダプタの中） |
#[derive(Debug, Clone)]
pub enum VideoCallBackend {
    Twilio(MockVideoCallProvider),
    Alt(MockAltVideoCallProvider),
}

impl VideoCallBackend {
    pub const PROVIDER_VAR: &'static str = "VIDEO_CALL_PROVIDER";

    /// プロセスの環境変数から選ぶ。
    pub fn from_env() -> Result<Self, UnknownVideoCallProvider> {
        Self::from_vars(|name| std::env::var(name).ok())
    }

    /// `var`（環境変数の名前から値）で選ぶ。知らない `VIDEO_CALL_PROVIDER` の値は [`UnknownVideoCallProvider`] で
    /// 失敗にする（黙って既定に倒さない）。
    pub fn from_vars(
        var: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, UnknownVideoCallProvider> {
        match var(Self::PROVIDER_VAR).as_deref() {
            None | Some("twilio") => Ok(Self::Twilio(MockVideoCallProvider::new())),
            Some("alt") => Ok(Self::Alt(MockAltVideoCallProvider::new())),
            Some(other) => Err(UnknownVideoCallProvider {
                value: other.to_string(),
            }),
        }
    }
}

impl Default for VideoCallBackend {
    fn default() -> Self {
        Self::Twilio(MockVideoCallProvider::new())
    }
}

impl VideoCallProvider for VideoCallBackend {
    async fn create_room(
        &self,
        session_id: SessionId,
        participant: AccountId,
    ) -> Result<String, Unavailable> {
        match self {
            Self::Twilio(provider) => provider.create_room(session_id, participant).await,
            Self::Alt(provider) => provider.create_room(session_id, participant).await,
        }
    }

    async fn close_room(&self, session_id: SessionId) -> Result<ConnectionQuality, Unavailable> {
        match self {
            Self::Twilio(provider) => provider.close_room(session_id).await,
            Self::Alt(provider) => provider.close_room(session_id).await,
        }
    }
}
