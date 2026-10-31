//! 動画の送付と添削の返却を、組み立て済みのルーター（`server::router_at`）で一周させる。動画の保存先は infra の
//! モック（`MockVideoStorage`）で、添削に着手してよいかは entitlement（`EntitlementReviewGate`）に問い合わせる。
//!
//! - 送付するとモックの保存先の URL が返り、返却前の取得は未返却（`in_progress`）
//! - 送付の直後に AI の一次フィードバックのモック（`MockAiFeedbackGenerator`）の文が付く
//! - コーチが返却すると、生徒が読む添削が `returned` になり、一次フィードバックはそのまま
//! - 生徒が送った動画の一覧（`GET /videos/me`）で、返却されたことと返却された時刻を確かめられる

use std::time::{Duration, SystemTime};

use application::{
    identity::{TokenClaims, TokenIssuer},
    in_memory::ManualClock,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{AccountId, Role, Sport};
use infra::{MockAiFeedbackGenerator, MockTokenService, MockVideoStorage};
use serde_json::{Value, json};
use tower::ServiceExt;

/// 2026-10-26T10:00:00Z
fn oct26_10() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800 + 10 * 3600)
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

fn get(path: &str, token: &str) -> Request<Body> {
    Request::get(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn a_submitted_video_is_reviewed_and_returned() {
    let clock = ManualClock::new(oct26_10());
    let router = server::router_at(clock.clone());
    let coach_id = AccountId::from_u128(42);
    let coach = MockTokenService
        .issue(TokenClaims {
            account_id: coach_id,
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

    let (status, video) = send(
        &router,
        post_json(
            "/videos",
            &student,
            json!({
                "coach_id": "00000000-0000-0000-0000-00000000002a",
                "sport": "tennis",
                "content_type": "video/quicktime"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{video}");
    let video_id = video["video_id"].as_str().unwrap();
    assert_eq!(
        video["upload_url"],
        format!("{}{video_id}", MockVideoStorage::URL_PREFIX)
    );
    let review_path = format!("/videos/{video_id}/review");

    // 返却前は未返却
    let (status, body) = send(&router, get(&review_path, &student)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "in_progress");
    assert_eq!(body["comment"], Value::Null);
    let ai_feedback = json!({
        "comment": MockAiFeedbackGenerator::comment_for(Sport::Tennis),
        "created_at": "2026-10-26T10:00:00Z",
    });
    assert_eq!(body["ai_feedback"], ai_feedback);

    // 翌日にコーチが返却する
    clock.set(oct26_10() + Duration::from_secs(86_400));
    let (status, body) = send(
        &router,
        post_json(
            &review_path,
            &coach,
            json!({ "comment": "フォロースルーを大きく" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = send(&router, get(&review_path, &student)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "returned");
    assert_eq!(body["comment"], "フォロースルーを大きく");
    assert_eq!(body["returned_at"], "2026-10-27T10:00:00Z");
    assert_eq!(body["ai_feedback"], ai_feedback);

    // 生徒は自分が送った動画の一覧で返却を確かめられる（コーチは呼べない）
    let (status, list) = send(&router, get("/videos/me", &student)).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list["videos"].as_array().unwrap().len(), 1, "{list}");
    assert_eq!(list["videos"][0]["video_id"], video_id);
    assert_eq!(list["videos"][0]["review_status"], "returned");
    assert_eq!(list["videos"][0]["returned_at"], "2026-10-27T10:00:00Z");
    let (status, _) = send(&router, get("/videos/me", &coach)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
