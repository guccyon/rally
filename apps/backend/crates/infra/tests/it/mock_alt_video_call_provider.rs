//! 別の通話基盤を想定したモック（`MockAltVideoCallProvider`）の検査。基盤の生の値（損失は 0〜1 の割合、ジッタと
//! 往復の遅延は秒）からポートの単位（百分率・ミリ秒）への換算を境界の値で確かめ、ルームの開閉と生成する接続品質の
//! 分布（Blueprint の「回線品質の低いセッションを 15% 混ぜる」）も見る。

use application::live_lesson::VideoCallProvider;
use domain::{
    identity::AccountId,
    live_lesson::{ConnectionQuality, SessionId},
};
use infra::MockAltVideoCallProvider;

const STUDENT: AccountId = AccountId::from_u128(1);

fn convert(
    packet_loss_ratio: f64,
    jitter_s: f64,
    round_trip_s: f64,
    disconnect_count: u32,
) -> Option<ConnectionQuality> {
    MockAltVideoCallProvider::to_connection_quality(
        packet_loss_ratio,
        jitter_s,
        round_trip_s,
        disconnect_count,
    )
    .ok()
}

#[test]
fn packet_loss_ratio_becomes_a_percentage() {
    for (ratio, percent) in [(0.0, 0.0), (0.01, 1.0), (0.5, 50.0), (1.0, 100.0)] {
        let quality = convert(ratio, 0.0, 0.0, 0).unwrap();
        assert!(
            (quality.packet_loss_rate() - percent).abs() < 1e-9,
            "{ratio}: {quality:?}"
        );
    }
    for ratio in [-0.0001, 1.0001, 100.0, f64::NAN, f64::INFINITY] {
        assert_eq!(convert(ratio, 0.0, 0.0, 0), None, "{ratio}");
    }
}

#[test]
fn seconds_become_rounded_milliseconds() {
    for (seconds, ms) in [
        (0.0, 0),
        (0.0004, 0),
        (0.0005, 1),
        (0.03, 30),
        (0.15, 150),
        (0.1504, 150),
        (0.1505, 151),
        (1.0, 1000),
        (f64::from(u32::MAX) / 1000.0, u32::MAX),
    ] {
        let quality = convert(0.0, seconds, seconds, 0).unwrap();
        assert_eq!(quality.jitter_ms(), ms, "ジッタ {seconds}");
        assert_eq!(quality.round_trip_ms(), ms, "往復 {seconds}");
    }
    // ジッタと往復の遅延は別々に換算する
    let quality = convert(0.0, 0.03, 0.15, 0).unwrap();
    assert_eq!((quality.jitter_ms(), quality.round_trip_ms()), (30, 150));
}

#[test]
fn broken_seconds_are_unavailable() {
    let too_long = (f64::from(u32::MAX) + 1.0) / 1000.0;
    for seconds in [-0.0001, -1.0, too_long, f64::NAN, f64::INFINITY] {
        assert_eq!(convert(0.0, seconds, 0.0, 0), None, "ジッタ {seconds}");
        assert_eq!(convert(0.0, 0.0, seconds, 0), None, "往復 {seconds}");
    }
}

#[test]
fn disconnect_count_is_kept_as_given() {
    assert_eq!(convert(0.0, 0.0, 0.0, 0).unwrap().disconnect_count(), 0);
    assert_eq!(convert(0.0, 0.0, 0.0, 3).unwrap().disconnect_count(), 3);
}

/// 回線品質が低いか（`mock_video_call_provider.rs` と同じ目安。ポートの単位で見る）。
fn is_low(quality: &ConnectionQuality) -> bool {
    quality.packet_loss_rate() > 1.0
        || quality.jitter_ms() > 30
        || quality.round_trip_ms() > 150
        || quality.disconnect_count() > 0
}

async fn qualities(provider: &MockAltVideoCallProvider, sessions: u128) -> Vec<ConnectionQuality> {
    let mut qualities = Vec::new();
    for id in 0..sessions {
        let session = SessionId::from_u128(id);
        provider.create_room(session, STUDENT).await.unwrap();
        qualities.push(provider.close_room(session).await.unwrap());
    }
    qualities
}

#[tokio::test]
async fn about_15_percent_of_sessions_have_low_quality_in_port_units() {
    assert_eq!(MockAltVideoCallProvider::LOW_QUALITY_RATIO, 0.15);
    // 4000 件の割合の標準偏差は約 0.0056。±0.03（5 標準偏差強）の幅で見る
    for seed in 1..=3 {
        let qualities = qualities(&MockAltVideoCallProvider::with_seed(seed), 4000).await;
        let ratio = qualities.iter().filter(|q| is_low(q)).count() as f64 / 4000.0;
        assert!((0.12..=0.18).contains(&ratio), "seed {seed}: {ratio}");
        for quality in &qualities {
            // 換算後はポートの単位（百分率・ミリ秒）の範囲に入る
            assert!(
                (0.0..=12.0).contains(&quality.packet_loss_rate()),
                "{quality:?}"
            );
            assert!((2..=150).contains(&quality.jitter_ms()), "{quality:?}");
            assert!((20..=600).contains(&quality.round_trip_ms()), "{quality:?}");
            if is_low(quality) {
                assert!(quality.packet_loss_rate() > 1.0, "{quality:?}");
                assert!(quality.jitter_ms() > 30, "{quality:?}");
                assert!(quality.round_trip_ms() > 150, "{quality:?}");
            }
        }
    }
}

#[tokio::test]
async fn the_same_seed_generates_the_same_qualities() {
    let first = qualities(&MockAltVideoCallProvider::with_seed(42), 50).await;
    let second = qualities(&MockAltVideoCallProvider::with_seed(42), 50).await;
    assert_eq!(first, second);
}

#[tokio::test]
async fn the_token_names_the_alt_room() {
    let provider = MockAltVideoCallProvider::with_seed(1);
    let session = SessionId::from_u128(0x0123_4567_89ab_4def_8123_4567_89ab_cdef);
    let token = provider.create_room(session, STUDENT).await.unwrap();
    assert_eq!(
        token,
        "mock-alt-room.01234567-89ab-4def-8123-456789abcdef.00000000-0000-0000-0000-000000000001"
    );
}
