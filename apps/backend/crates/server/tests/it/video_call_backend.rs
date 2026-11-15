//! 通話基盤を環境変数で選ぶ（`VideoCallBackend`。変更シナリオ cs-007）。選んだ基盤で組み立てたルーターで通話を
//! 開始・終了し、`room_token` の形でどちらの基盤が使われたかを確かめる。

use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};

use application::{
    identity::{TokenClaims, TokenIssuer},
    in_memory::{ManualClock, RecordingAnalyticsSink},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{AccountId, Role};
use infra::MockTokenService;
use serde_json::{Value, json};
use server::{UnknownVideoCallProvider, VideoCallBackend};
use tower::ServiceExt;

fn choose(vars: &[(&str, &str)]) -> Result<VideoCallBackend, UnknownVideoCallProvider> {
    let vars: HashMap<String, String> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    VideoCallBackend::from_vars(|name| vars.get(name).cloned())
}

#[test]
fn twilio_is_the_default() {
    assert!(matches!(choose(&[]), Ok(VideoCallBackend::Twilio(_))));
    assert!(matches!(
        choose(&[("VIDEO_CALL_PROVIDER", "twilio")]),
        Ok(VideoCallBackend::Twilio(_))
    ));
}

#[test]
fn alt_chooses_the_other_backend() {
    assert!(matches!(
        choose(&[("VIDEO_CALL_PROVIDER", "alt")]),
        Ok(VideoCallBackend::Alt(_))
    ));
}

#[test]
fn unknown_provider_is_an_error() {
    for value in ["", "Twilio", "ALT", "zoom"] {
        let error = choose(&[("VIDEO_CALL_PROVIDER", value)]).unwrap_err();
        assert_eq!(
            error,
            UnknownVideoCallProvider {
                value: value.to_string()
            }
        );
        assert!(error.to_string().contains("VIDEO_CALL_PROVIDER"), "{error}");
    }
}

/// 2026-11-01T10:00:00Z（スロットの開始）
fn nov1_10() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200 + 10 * 3600)
}

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn post_json(path: &str, token: &str, body: Value) -> Request<Body> {
    Request::post(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// `vars` で選んだ通話基盤で組み立てたルーターで、確定した予約の通話を開始して終了し、開始の応答の `room_token` を返す。
async fn room_token_of_a_call(vars: &[(&str, &str)]) -> String {
    let clock = ManualClock::new(nov1_10() - Duration::from_secs(86_400));
    let router = server::router_with_video_call(
        clock.clone(),
        RecordingAnalyticsSink::default(),
        choose(vars).unwrap(),
    );
    let coach = MockTokenService
        .issue(TokenClaims {
            account_id: AccountId::from_u128(42),
            role: Role::Coach,
        })
        .into_string();
    let (status, account) = send(
        &router,
        Request::post("/accounts")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "tennis" })
                    .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{account}");
    let student = account["access_token"].as_str().unwrap().to_string();
    let (status, slot) = send(
        &router,
        post_json(
            "/slots",
            &coach,
            json!({
                "sport": "tennis",
                "starts_at": "2026-11-01T10:00:00Z",
                "ends_at": "2026-11-01T11:00:00Z"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{slot}");
    let (status, booking) = send(
        &router,
        post_json(
            "/bookings",
            &student,
            json!({ "slot_id": slot["slot_id"], "is_trial": true }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");
    let booking_id = booking["booking_id"].as_str().unwrap();
    let (status, body) = send(
        &router,
        post_json(
            &format!("/bookings/{booking_id}/confirm"),
            &student,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    clock.set(nov1_10());
    let (status, session) = send(
        &router,
        post_json(
            "/live-lesson-sessions",
            &coach,
            json!({ "booking_id": booking_id }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{session}");
    let session_id = session["session_id"].as_str().unwrap();

    // どちらの基盤でも終了でき、応答の形は同じ
    clock.set(nov1_10() + Duration::from_secs(45 * 60));
    let (status, ended) = send(
        &router,
        post_json(
            &format!("/live-lesson-sessions/{session_id}/end"),
            &student,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ended}");
    assert_eq!(ended["status"], "ended");
    assert_eq!(ended["duration_sec"], 45 * 60);

    session["room_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn the_default_router_uses_the_twilio_mock() {
    let token = room_token_of_a_call(&[]).await;
    assert!(token.starts_with("mock-room."), "{token}");
}

#[tokio::test]
async fn alt_router_uses_the_alt_mock() {
    let token = room_token_of_a_call(&[("VIDEO_CALL_PROVIDER", "alt")]).await;
    assert!(token.starts_with("mock-alt-room."), "{token}");
}
