use std::future::Future;

use domain::{
    identity::AccountId,
    live_lesson::{ConnectionQuality, SessionId},
};

use crate::Unavailable;

/// 通話基盤（Twilio Video 相当）のポート。ルームは通話セッション 1 つにつき 1 つで、セッション ID で指す。
///
/// Gen1 の実装は infra のモック（`MockVideoCallProvider`。接続品質の指標を乱数で生成する）と、テスト用の
/// [`crate::in_memory::FixedVideoCallProvider`]。録画は扱わない。
pub trait VideoCallProvider: Send + Sync {
    /// セッション `session_id` の通話ルームを作り、参加者 `participant` がルームに参加するためのトークン
    /// （クライアントには不透明）を返す。
    fn create_room(
        &self,
        session_id: SessionId,
        participant: AccountId,
    ) -> impl Future<Output = Result<String, Unavailable>> + Send;

    /// セッション `session_id` のルームを閉じ、通話の接続品質の生の指標を返す。
    fn close_room(
        &self,
        session_id: SessionId,
    ) -> impl Future<Output = Result<ConnectionQuality, Unavailable>> + Send;
}
