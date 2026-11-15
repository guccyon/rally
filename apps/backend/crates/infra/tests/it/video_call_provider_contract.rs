//! ポート `VideoCallProvider` の契約を、通話基盤のモックのどちらも（Twilio Video 相当の `MockVideoCallProvider`、
//! 別の基盤を想定した `MockAltVideoCallProvider`）と、それを選ぶ `VideoCallBackend` が満たすこと（変更シナリオ cs-007）。
//!
//! - ルームを作るとトークン（空でない文字列）を返し、参加者ごとに違う
//! - 開いていないルームは閉じられない。閉じたルームは 2 度は閉じられない
//! - 閉じると接続品質をポートの単位（損失は百分率 0〜100、ジッタと往復の遅延はミリ秒）で返す
//! - 複製は同じルームを共有する

use application::live_lesson::VideoCallProvider;
use domain::{identity::AccountId, live_lesson::SessionId};
use infra::{MockAltVideoCallProvider, MockVideoCallProvider, VideoCallBackend};

const STUDENT: AccountId = AccountId::from_u128(1);
const COACH: AccountId = AccountId::from_u128(2);

async fn satisfies_the_port<P: VideoCallProvider + Clone>(provider: P) {
    let session = SessionId::from_u128(7);

    assert!(provider.close_room(session).await.is_err(), "開いていない");

    let student = provider.create_room(session, STUDENT).await.unwrap();
    let coach = provider.create_room(session, COACH).await.unwrap();
    assert!(!student.is_empty());
    assert_ne!(student, coach, "参加者ごとに違うトークン");

    let quality = provider.clone().close_room(session).await.unwrap();
    assert!(
        (0.0..=100.0).contains(&quality.packet_loss_rate()),
        "{quality:?}"
    );
    // ミリ秒で返す（秒のまま返すと良好な回線でも 1 未満に潰れる）。モックの分布はどちらも 2 ms 以上・600 ms 以下
    assert!((2..=150).contains(&quality.jitter_ms()), "{quality:?}");
    assert!((20..=600).contains(&quality.round_trip_ms()), "{quality:?}");

    assert!(provider.close_room(session).await.is_err(), "閉じた後");

    // 多数のセッションでも単位が崩れない
    for id in 100..600 {
        let session = SessionId::from_u128(id);
        provider.create_room(session, STUDENT).await.unwrap();
        let quality = provider.close_room(session).await.unwrap();
        assert!(
            (0.0..=100.0).contains(&quality.packet_loss_rate()),
            "{quality:?}"
        );
        assert!((2..=150).contains(&quality.jitter_ms()), "{quality:?}");
        assert!((20..=600).contains(&quality.round_trip_ms()), "{quality:?}");
    }
}

#[tokio::test]
async fn twilio_mock_satisfies_the_port() {
    satisfies_the_port(MockVideoCallProvider::with_seed(1)).await;
}

#[tokio::test]
async fn alt_mock_satisfies_the_port() {
    satisfies_the_port(MockAltVideoCallProvider::with_seed(1)).await;
}

#[tokio::test]
async fn both_choices_of_the_backend_satisfy_the_port() {
    let twilio = MockVideoCallProvider::with_seed(2);
    satisfies_the_port(VideoCallBackend::Twilio(twilio)).await;
    let alt = MockAltVideoCallProvider::with_seed(2);
    satisfies_the_port(VideoCallBackend::Alt(alt)).await;
}
