//! 行動トラッキングの配信先のモック（`ObjectStoreAnalyticsSink`）が、イベントを JSON Lines でオブジェクトストアに書くか。

use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use application::analytics::{AnalyticsEvent, AnalyticsSink};
use axum::{
    Router,
    body::Bytes,
    http::{HeaderMap, Method, StatusCode, Uri},
    response::IntoResponse,
};
use domain::{
    identity::{AccountId, Sport},
    scheduling::SlotId,
};
use infra::{ObjectStoreAnalyticsSink, analytics_event_payload::line};
use object_store::{ObjectStore, memory::InMemory, path::Path};
use uuid::Uuid;

/// 2026-10-26T23:59:59Z（UTC の日付で区切ることを見るため、日本時間では翌日になる時刻）
fn oct26_late() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800 + 86_399)
}

fn registered() -> AnalyticsEvent {
    AnalyticsEvent::AccountRegistered {
        student_id: AccountId::from_u128(1),
        sport: Sport::Tennis,
        occurred_at: oct26_late(),
    }
}

fn published() -> AnalyticsEvent {
    AnalyticsEvent::SlotPublished {
        coach_id: AccountId::from_u128(2),
        slot_id: SlotId::from_u128(3),
        starts_at: oct26_late(),
        occurred_at: oct26_late(),
    }
}

#[test]
fn key_is_partitioned_by_the_event_name_and_the_utc_date() {
    let id = Uuid::from_u128(0x0123_4567_89ab_4def_8123_4567_89ab_cdef);
    assert_eq!(
        ObjectStoreAnalyticsSink::key(&registered(), id)
            .unwrap()
            .as_ref(),
        "events/account.registered/dt=2026-10-26/01234567-89ab-4def-8123-456789abcdef.jsonl"
    );
}

/// `prefix` の直下のオブジェクトの中身。
async fn objects(store: &InMemory, prefix: &str) -> Vec<String> {
    let listed = store
        .list_with_delimiter(Some(&Path::from(prefix)))
        .await
        .unwrap();
    let mut bodies = Vec::new();
    for object in listed.objects {
        assert!(object.location.as_ref().ends_with(".jsonl"));
        let bytes = store.get_opts(&object.location, Default::default()).await;
        let bytes = bytes.unwrap().bytes().await.unwrap();
        bodies.push(String::from_utf8(bytes.to_vec()).unwrap());
    }
    bodies
}

#[tokio::test]
async fn emit_writes_one_json_line_per_event_to_the_object_store() {
    let store = Arc::new(InMemory::new());
    let sink = ObjectStoreAnalyticsSink::new(store.clone());

    sink.emit(registered()).await;
    sink.emit(registered()).await;
    sink.emit(published()).await;

    let registered_objects = objects(&store, "events/account.registered/dt=2026-10-26").await;
    assert_eq!(registered_objects.len(), 2, "発火ごとに 1 オブジェクト");
    for body in registered_objects {
        assert_eq!(body, format!("{}\n", line(&registered()).unwrap()));
    }
    assert_eq!(
        objects(&store, "events/slot.published/dt=2026-10-26").await,
        [format!("{}\n", line(&published()).unwrap())]
    );
}

/// 受け取った PUT（パスと本文）。
type Received = Arc<Mutex<Vec<(String, String)>>>;

/// S3 API の PUT Object だけに答える偽物のオブジェクトストア。
async fn fake_s3() -> (String, Received) {
    let received: Received = Arc::default();
    let log = received.clone();
    let app = Router::new().fallback(
        move |method: Method, uri: Uri, headers: HeaderMap, body: Bytes| {
            let log = log.clone();
            async move {
                if method != Method::PUT {
                    return StatusCode::METHOD_NOT_ALLOWED.into_response();
                }
                assert!(
                    !headers.contains_key("authorization"),
                    "モックの配信先には署名しない"
                );
                log.lock().unwrap().push((
                    uri.path().to_string(),
                    String::from_utf8(body.to_vec()).unwrap(),
                ));
                (StatusCode::OK, [("etag", "\"mock\"")]).into_response()
            }
        },
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await });
    (endpoint, received)
}

#[tokio::test]
async fn s3_puts_the_object_into_the_bucket_by_path_style() {
    let (endpoint, received) = fake_s3().await;
    let sink = ObjectStoreAnalyticsSink::s3(&endpoint, "rally-analytics").unwrap();

    sink.emit(published()).await;

    let received = received.lock().unwrap().clone();
    assert_eq!(received.len(), 1, "{received:?}");
    let (path, body) = &received[0];
    // キーの `=` は URL の中でパーセント符号化される（S3 側で戻る）
    let path = path.replace("%3D", "=");
    let key = path
        .strip_prefix("/rally-analytics/events/slot.published/dt=2026-10-26/")
        .unwrap_or_else(|| panic!("{path}"));
    let id = key
        .strip_suffix(".jsonl")
        .unwrap_or_else(|| panic!("{key}"));
    assert_eq!(Uuid::try_parse(id).unwrap().get_version_num(), 4);
    assert_eq!(body, &format!("{}\n", line(&published()).unwrap()));
}

/// 配信先が止まっていても、発火は失敗を標準エラー出力に書いて数秒で戻る（ユースケースを失敗させない）。
#[tokio::test]
async fn emit_gives_up_quickly_when_the_object_store_is_down() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let sink = ObjectStoreAnalyticsSink::s3(&endpoint, "rally-analytics").unwrap();

    let started = std::time::Instant::now();
    sink.emit(published()).await;
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
}
