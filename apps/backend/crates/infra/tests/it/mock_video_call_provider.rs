//! 通話基盤のモック（`MockVideoCallProvider`）の検査。ルームの開閉と、終了時に生成する接続品質の分布
//! （Blueprint の「回線品質の低いセッションを 15% 混ぜる」）を確かめる。分布は統計的に幅を持たせて見る。

use application::live_lesson::VideoCallProvider;
use domain::{
    identity::AccountId,
    live_lesson::{ConnectionQuality, SessionId},
};
use infra::MockVideoCallProvider;

const STUDENT: AccountId = AccountId::from_u128(1);

/// 回線品質が低いか。ジッタ 30 ms・往復 150 ms の目安（docs/adr/0002 Q1。ITU-T G.114 / G.1010）を超えるか、
/// パケットを 1% より多く失ったか、切断したか。スコアの式（セマンティック層）には依らない。
fn is_low(quality: &ConnectionQuality) -> bool {
    quality.packet_loss_rate() > 1.0
        || quality.jitter_ms() > 30
        || quality.round_trip_ms() > 150
        || quality.disconnect_count() > 0
}

/// `sessions` 件のセッションのルームを開いて閉じ、生成された接続品質を返す。
async fn qualities(provider: &MockVideoCallProvider, sessions: u128) -> Vec<ConnectionQuality> {
    let mut qualities = Vec::new();
    for id in 0..sessions {
        let session = SessionId::from_u128(id);
        provider.create_room(session, STUDENT).await.unwrap();
        qualities.push(provider.close_room(session).await.unwrap());
    }
    qualities
}

fn low_ratio(qualities: &[ConnectionQuality]) -> f64 {
    qualities.iter().filter(|q| is_low(q)).count() as f64 / qualities.len() as f64
}

#[tokio::test]
async fn about_15_percent_of_sessions_have_low_quality() {
    assert_eq!(MockVideoCallProvider::LOW_QUALITY_RATIO, 0.15);
    // 4000 件の割合の標準偏差は約 0.0056。±0.03（5 標準偏差強）の幅で見る
    let mut all = Vec::new();
    for seed in 1..=5 {
        let qualities = qualities(&MockVideoCallProvider::with_seed(seed), 4000).await;
        let ratio = low_ratio(&qualities);
        assert!((0.12..=0.18).contains(&ratio), "seed {seed}: {ratio}");
        all.extend(qualities);
    }
    let ratio = low_ratio(&all);
    assert!((0.135..=0.165).contains(&ratio), "20000 件: {ratio}");

    // OS の乱数で初期化しても同じ分布（種を固定しないので幅を広めに取る。±0.04 は 7 標準偏差強）
    let ratio = low_ratio(&qualities(&MockVideoCallProvider::new(), 4000).await);
    assert!((0.11..=0.19).contains(&ratio), "OS の乱数: {ratio}");
}

#[tokio::test]
async fn low_quality_sessions_exceed_every_guideline_and_good_ones_stay_within() {
    let qualities = qualities(&MockVideoCallProvider::with_seed(7), 2000).await;
    for quality in &qualities {
        if is_low(quality) {
            // 品質の低い回線は、損失・ジッタ・往復の遅延のどれも目安を超える（切断は 0 のこともある）
            assert!(quality.packet_loss_rate() > 1.0, "{quality:?}");
            assert!(quality.jitter_ms() > 30, "{quality:?}");
            assert!(quality.round_trip_ms() > 150, "{quality:?}");
        }
        assert!(
            (0.0..=100.0).contains(&quality.packet_loss_rate()),
            "{quality:?}"
        );
    }
    // 切断が起きるセッションもある（品質の低い回線だけ）
    assert!(qualities.iter().any(|q| q.disconnect_count() > 0));
}

#[tokio::test]
async fn the_same_seed_generates_the_same_qualities() {
    let first = qualities(&MockVideoCallProvider::with_seed(42), 50).await;
    let second = qualities(&MockVideoCallProvider::with_seed(42), 50).await;
    assert_eq!(first, second);
}

#[tokio::test]
async fn rooms_must_be_open_to_be_closed() {
    let provider = MockVideoCallProvider::with_seed(1);
    let session = SessionId::from_u128(0x0123_4567_89ab_4def_8123_4567_89ab_cdef);

    assert!(provider.close_room(session).await.is_err(), "開いていない");
    let token = provider.create_room(session, STUDENT).await.unwrap();
    assert_eq!(
        token,
        "mock-room.01234567-89ab-4def-8123-456789abcdef.00000000-0000-0000-0000-000000000001"
    );
    // 複製は同じルームを共有する
    provider.clone().close_room(session).await.unwrap();
    assert!(provider.close_room(session).await.is_err(), "閉じた後");
}
