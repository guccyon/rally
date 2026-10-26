//! 通話基盤（Twilio Video）のモック（Gen1 の外部連携はすべてモック）。

use std::{
    collections::HashSet,
    ops::RangeInclusive,
    sync::{Arc, Mutex},
};

use application::{
    Unavailable, identity::account_id_uuid, live_lesson::VideoCallProvider,
    live_lesson::session_id_uuid,
};
use domain::{
    identity::AccountId,
    live_lesson::{ConnectionQuality, SessionId},
};
use rand::{RngExt, SeedableRng, rngs::StdRng};

/// 接続品質の指標を生成する分布の 1 つ（回線の状態）。各指標は範囲の中の一様分布。
struct Profile {
    /// 失われたパケットの百分率
    packet_loss_rate: RangeInclusive<f64>,
    jitter_ms: RangeInclusive<u32>,
    round_trip_ms: RangeInclusive<u32>,
    disconnect_count: RangeInclusive<u32>,
}

/// 良好な回線。ジッタ・往復の遅延は ITU-T G.114 / G.1010 の目安（30 ms・150 ms。docs/adr/0002 Q1）以内。
const GOOD: Profile = Profile {
    packet_loss_rate: 0.0..=1.0,
    jitter_ms: 2..=30,
    round_trip_ms: 20..=150,
    disconnect_count: 0..=0,
};

/// 品質の低い回線。損失・ジッタ・往復の遅延のどれも良好な回線の範囲と重ならない（目安を超える）。
const LOW: Profile = Profile {
    packet_loss_rate: 2.0..=12.0,
    jitter_ms: 40..=150,
    round_trip_ms: 200..=600,
    disconnect_count: 0..=3,
};

/// [`VideoCallProvider`] のモック。ルームは同じプロセスのメモリにだけ持ち、実際の通話はしない。
///
/// ルームを閉じると接続品質の指標を乱数で生成して返す。Blueprint の「回線品質の低いセッションを 15% 混ぜる」に
/// 合わせ、[`MockVideoCallProvider::LOW_QUALITY_RATIO`] の確率で品質の低い回線の分布から、それ以外は良好な
/// 回線の分布から引く。通話品質スコアは計算しない（docs/adr/0002 Q1）。
///
/// トークンは `mock-room.<セッション ID>.<参加者 ID>` で、署名はない。複製は同じルームと乱数列を共有する。
#[derive(Debug, Clone)]
pub struct MockVideoCallProvider {
    state: Arc<Mutex<State>>,
}

#[derive(Debug)]
struct State {
    rng: StdRng,
    rooms: HashSet<SessionId>,
}

impl MockVideoCallProvider {
    /// 品質の低い回線のセッションの割合（Blueprint）。
    pub const LOW_QUALITY_RATIO: f64 = 0.15;

    /// OS の乱数で初期化する。
    pub fn new() -> Self {
        Self::with_rng(rand::make_rng())
    }

    /// 乱数の種を固定する（テストで同じ列を再現する）。
    pub fn with_seed(seed: u64) -> Self {
        Self::with_rng(StdRng::seed_from_u64(seed))
    }

    fn with_rng(rng: StdRng) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                rng,
                rooms: HashSet::new(),
            })),
        }
    }
}

impl Default for MockVideoCallProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    fn generate_quality(&mut self) -> Result<ConnectionQuality, Unavailable> {
        let rng = &mut self.rng;
        let profile = if rng.random_bool(MockVideoCallProvider::LOW_QUALITY_RATIO) {
            &LOW
        } else {
            &GOOD
        };
        ConnectionQuality::new(
            rng.random_range(profile.packet_loss_rate.clone()),
            rng.random_range(profile.jitter_ms.clone()),
            rng.random_range(profile.round_trip_ms.clone()),
            rng.random_range(profile.disconnect_count.clone()),
        )
        .map_err(Unavailable::new)
    }
}

impl VideoCallProvider for MockVideoCallProvider {
    async fn create_room(
        &self,
        session_id: SessionId,
        participant: AccountId,
    ) -> Result<String, Unavailable> {
        self.state.lock().unwrap().rooms.insert(session_id);
        Ok(format!(
            "mock-room.{}.{}",
            session_id_uuid::format(session_id),
            account_id_uuid::format(participant)
        ))
    }

    async fn close_room(&self, session_id: SessionId) -> Result<ConnectionQuality, Unavailable> {
        let mut state = self.state.lock().unwrap();
        if !state.rooms.contains(&session_id) {
            return Err(Unavailable::new(format!(
                "ルーム {} が開いていない",
                session_id_uuid::format(session_id)
            )));
        }
        let quality = state.generate_quality()?;
        state.rooms.remove(&session_id);
        Ok(quality)
    }
}
