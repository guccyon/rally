//! 一周シナリオの終点（レッスンの完了記録）を、組み立て済みのルーター（`server::router_at`）で通す。
//! 通話基盤・動画の保存先・トークンは infra のモックで、完了記録は通話セッション・動画の保存先と同じ中身を読む。
//!
//! - 体験予約 → 確定 → 通話の開始・終了 → 完了の記録で、`is_first_lesson` が true になる
//! - 同じ生徒が動画を送り、添削が返されてから完了を記録すると、2 回目なので `is_first_lesson` は false
//! - 生徒が完了したレッスンを 1 回だけ評価でき、本部とコーチ本人がコーチの評価の件数と平均を読める

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
use domain::identity::{AccountId, Role};
use infra::MockTokenService;
use serde_json::{Value, json};
use tower::ServiceExt;

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

/// `token` で `path` に POST し、`expected` の status であることを確かめて本文を返す。
async fn post_ok(
    router: &Router,
    path: &str,
    token: &str,
    body: Value,
    expected: StatusCode,
) -> Value {
    let (status, body) = send(router, post_json(path, token, body)).await;
    assert_eq!(status, expected, "{path}: {body}");
    body
}

#[tokio::test]
async fn live_and_async_lessons_are_completed_and_only_the_first_is_the_first_lesson() {
    let clock = ManualClock::new(nov1_10() - Duration::from_secs(86_400));
    let router = server::router_at(clock.clone());
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
    let student_id = account["student"]["student_id"].clone();
    assert!(student_id.is_string(), "{account}");

    // 体験予約 → 確定 → 通話
    let slot = post_ok(
        &router,
        "/slots",
        &coach,
        json!({
            "sport": "tennis",
            "starts_at": "2026-11-01T10:00:00Z",
            "ends_at": "2026-11-01T11:00:00Z"
        }),
        StatusCode::CREATED,
    )
    .await;
    let booking = post_ok(
        &router,
        "/bookings",
        &student,
        json!({ "slot_id": slot["slot_id"], "is_trial": true }),
        StatusCode::CREATED,
    )
    .await;
    let booking_id = booking["booking_id"].as_str().unwrap();
    post_ok(
        &router,
        &format!("/bookings/{booking_id}/confirm"),
        &student,
        json!({}),
        StatusCode::OK,
    )
    .await;
    clock.set(nov1_10());
    let session = post_ok(
        &router,
        "/live-lesson-sessions",
        &coach,
        json!({ "booking_id": booking_id }),
        StatusCode::CREATED,
    )
    .await;
    let session_id = session["session_id"].as_str().unwrap();
    let live = json!({ "lesson_type": "live_lesson", "session_id": session_id });

    // 終了する前は完了にできない
    let (status, body) = send(
        &router,
        post_json("/lessons/completions", &student, live.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "invalid_session_state");

    clock.set(nov1_10() + Duration::from_secs(45 * 60));
    post_ok(
        &router,
        &format!("/live-lesson-sessions/{session_id}/end"),
        &student,
        json!({}),
        StatusCode::OK,
    )
    .await;
    let first = post_ok(
        &router,
        "/lessons/completions",
        &coach,
        live,
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(first["lesson_type"], "live_lesson");
    assert_eq!(first["session_id"], session_id);
    assert_eq!(first["student_id"], student_id);
    assert_eq!(first["is_first_lesson"], true);
    assert_eq!(first["completed_at"], "2026-11-01T10:45:00Z");

    // 同じ生徒が動画を送り、翌日に添削が返される
    let video = post_ok(
        &router,
        "/videos",
        &student,
        json!({
            "coach_id": "00000000-0000-0000-0000-00000000002a",
            "sport": "tennis",
            "content_type": "video/mp4"
        }),
        StatusCode::CREATED,
    )
    .await;
    let video_id = video["video_id"].as_str().unwrap();
    clock.set(nov1_10() + Duration::from_secs(86_400));
    post_ok(
        &router,
        &format!("/videos/{video_id}/review"),
        &coach,
        json!({ "comment": "フォロースルーを大きく" }),
        StatusCode::OK,
    )
    .await;
    let second = post_ok(
        &router,
        "/lessons/completions",
        &student,
        json!({ "lesson_type": "async_lesson", "video_id": video_id }),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(second["lesson_type"], "async_lesson");
    assert_eq!(second["video_id"], video_id);
    assert_eq!(second["student_id"], student_id);
    assert_eq!(second["is_first_lesson"], false, "同じ生徒の 2 回目");

    // 生徒が通話のレッスンを評価する。2 回目は 409、コーチは評価できない
    let lesson_id = first["lesson_id"].as_str().unwrap();
    let review = json!({ "lesson_id": lesson_id, "rating": 4, "comment": "わかりやすかった" });
    let reviewed = post_ok(
        &router,
        "/lesson-reviews",
        &student,
        review.clone(),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(reviewed["lesson_id"], lesson_id);
    assert_eq!(reviewed["coach_id"], "00000000-0000-0000-0000-00000000002a");
    assert_eq!(reviewed["rating"], 4);
    let (status, body) = send(
        &router,
        post_json("/lesson-reviews", &student, review.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "lesson_already_reviewed");
    let (status, _) = send(&router, post_json("/lesson-reviews", &coach, review)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // 本部とコーチ本人がコーチの評価の件数と平均を読める
    let headquarters = MockTokenService
        .issue(TokenClaims {
            account_id: AccountId::from_u128(5),
            role: Role::Headquarters,
        })
        .into_string();
    for token in [&headquarters, &coach] {
        let (status, summary) = send(
            &router,
            Request::get("/coaches/00000000-0000-0000-0000-00000000002a/review-summary")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{summary}");
        assert_eq!(summary["review_count"], 1);
        assert_eq!(summary["average_rating"], 4.0);
    }
}
