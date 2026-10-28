//! 別の通話基盤（Twilio Video ではないもの）を想定したモック（変更シナリオ cs-007。Gen1 の外部連携はすべてモック）。

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

/// この基盤の生の値を生成する分布の 1 つ（回線の状態）。各指標は範囲の中の一様分布で、単位は基盤のもの。
struct Profile {
    /// 失われたパケットの割合（0〜1）
    packet_loss_ratio: RangeInclusive<f64>,
    /// ジッタ（秒）
    jitter_s: RangeInclusive<f64>,
    /// 往復の遅延（秒）
    round_trip_s: RangeInclusive<f64>,
    disconnect_count: RangeInclusive<u32>,
}

/// 良好な回線。`MockVideoCallProvider` の良好な回線と同じ範囲を、この基盤の単位で書いたもの
/// （ジッタ・往復の遅延は ITU-T G.114 / G.1010 の目安 30 ms・150 ms 以内。docs/adr/0002 Q1）。
const GOOD: Profile = Profile {
    packet_loss_ratio: 0.0..=0.01,
    jitter_s: 0.002..=0.030,
    round_trip_s: 0.020..=0.150,
    disconnect_count: 0..=0,
};

/// 品質の低い回線。損失・ジッタ・往復の遅延のどれも良好な回線の範囲と重ならない（目安を超える）。
const LOW: Profile = Profile {
    packet_loss_ratio: 0.02..=0.12,
    jitter_s: 0.040..=0.150,
    round_trip_s: 0.200..=0.600,
    disconnect_count: 0..=3,
};

/// 別の通話基盤を想定した [`VideoCallProvider`] のモック。ルームは同じプロセスのメモリにだけ持ち、実際の通話はしない。
///
/// この基盤はルームを閉じると、パケットの損失を 0〜1 の割合、ジッタと往復の遅延を秒で返す想定にする（Twilio Video
/// 相当の `MockVideoCallProvider` とは単位が違う）。ポートの単位（百分率・ミリ秒）への換算はこのアダプタの中だけで
/// 行う（[`MockAltVideoCallProvider::to_connection_quality`]）。生の値の分布は `MockVideoCallProvider` と同じで、
/// [`MockAltVideoCallProvider::LOW_QUALITY_RATIO`] の確率で品質の低い回線から引く。通話品質スコアは計算しない
/// （docs/adr/0002 Q1）。
///
/// トークンは `mock-alt-room.<セッション ID>.<参加者 ID>` で、署名はない。複製は同じルームと乱数列を共有する。
#[derive(Debug, Clone)]
pub struct MockAltVideoCallProvider {
    state: Arc<Mutex<State>>,
}

#[derive(Debug)]
struct State {
    rng: StdRng,
    rooms: HashSet<SessionId>,
}

impl MockAltVideoCallProvider {
    /// 品質の低い回線のセッションの割合（Blueprint の「回線品質の低いセッションを 15% 混ぜる」）。
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

    /// この基盤の生の値（損失は 0〜1 の割合、ジッタと往復の遅延は秒）を、ポートの単位の接続品質（損失は百分率、
    /// ジッタと往復の遅延はミリ秒）に換算する。ミリ秒は四捨五入する。
    ///
    /// 損失が 0〜1 の有限の値でない、または秒が 0 以上の有限の値でないか `u32` のミリ秒に収まらないときは
    /// [`Unavailable`]（基盤が壊れた値を返した）。
    pub fn to_connection_quality(
        packet_loss_ratio: f64,
        jitter_s: f64,
        round_trip_s: f64,
        disconnect_count: u32,
    ) -> Result<ConnectionQuality, Unavailable> {
        if !(0.0..=1.0).contains(&packet_loss_ratio) {
            return Err(Unavailable::new(format!(
                "パケットの損失の割合 {packet_loss_ratio} が 0〜1 にない"
            )));
        }
        ConnectionQuality::new(
            packet_loss_ratio * 100.0,
            seconds_to_ms(jitter_s, "ジッタ")?,
            seconds_to_ms(round_trip_s, "往復の遅延")?,
            disconnect_count,
        )
        .map_err(Unavailable::new)
    }
}

/// 秒をミリ秒に四捨五入する。
fn seconds_to_ms(seconds: f64, what: &str) -> Result<u32, Unavailable> {
    let ms = (seconds * 1000.0).round();
    if seconds >= 0.0 && ms <= f64::from(u32::MAX) {
        Ok(ms as u32)
    } else {
        Err(Unavailable::new(format!(
            "{what} {seconds} 秒がミリ秒の範囲にない"
        )))
    }
}

impl Default for MockAltVideoCallProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    fn generate_quality(&mut self) -> Result<ConnectionQuality, Unavailable> {
        let rng = &mut self.rng;
        let profile = if rng.random_bool(MockAltVideoCallProvider::LOW_QUALITY_RATIO) {
            &LOW
        } else {
            &GOOD
        };
        MockAltVideoCallProvider::to_connection_quality(
            rng.random_range(profile.packet_loss_ratio.clone()),
            rng.random_range(profile.jitter_s.clone()),
            rng.random_range(profile.round_trip_s.clone()),
            rng.random_range(profile.disconnect_count.clone()),
        )
    }
}

impl VideoCallProvider for MockAltVideoCallProvider {
    async fn create_room(
        &self,
        session_id: SessionId,
        participant: AccountId,
    ) -> Result<String, Unavailable> {
        self.state.lock().unwrap().rooms.insert(session_id);
        Ok(format!(
            "mock-alt-room.{}.{}",
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
