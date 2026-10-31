//! 変更シナリオ cs-006 の AI の一次フィードバックを、保存先を Postgres にした組み立て（`server::router_with_postgres`）で
//! 一周させる。生徒が動画を送ると、AI のモック（`MockAiFeedbackGenerator`）がスポーツごとの決まった文を返し、
//! 動画に保存される。生徒は `getVideoReview`、コーチは `listMyCoachVideos` で読め、コーチの添削の返却で上書きされない。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test ai_feedback -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    identity::{RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer},
    in_memory::{ManualClock, RecordingAnalyticsSink},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{catalog::Sport, identity::Role};
use infra::{MockAiFeedbackGenerator, MockTokenService, PgAccountRepository};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

/// 2026-10-26T10:00:00Z
fn oct26_10() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800 + 10 * 3600)
}

async fn send(
    router: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let body = body.map_or_else(Body::empty, |b| Body::from(b.to_string()));
    let response = router
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

/// 水泳のコーチを登録して ID とトークンを返す。契約にコーチの登録の操作がないので、ユースケースを直に呼ぶ。
async fn swimming_coach(pool: &PgPool) -> (String, String) {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        ManualClock::new(oct26_10()),
    )
    .execute(RegisterAccountCommand {
        email: "coach@example.com".to_string(),
        display_name: "およぐ".to_string(),
        role: Role::Coach,
        sports: vec![Sport::Swimming],
    })
    .await
    .unwrap();
    let token = MockTokenService
        .issue(TokenClaims {
            account_id: account.id(),
            role: Role::Coach,
        })
        .into_string();
    (
        application::identity::account_id_uuid::format(account.id()),
        token,
    )
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_submitted_video_gets_ai_feedback_that_the_coachs_review_does_not_overwrite(
    pool: PgPool,
) {
    let (coach_id, coach) = swimming_coach(&pool).await;
    let clock = ManualClock::new(oct26_10());
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    let (status, account) = send(
        &router,
        "POST",
        "/accounts",
        None,
        Some(json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "swimming" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{account}");
    let student = account["access_token"].as_str().unwrap().to_string();

    // 生徒が一言を添えて動画を送る。送付は成功し、その直後に AI の一次フィードバックが付く
    let (status, video) = send(
        &router,
        "POST",
        "/videos",
        Some(&student),
        Some(json!({
            "coach_id": coach_id,
            "sport": "swimming",
            "content_type": "video/mp4",
            "note": "クロールの息継ぎを見てほしい"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{video}");
    let video_id = video["video_id"].as_str().unwrap().to_string();
    let review_path = format!("/videos/{video_id}/review");
    let expected = json!({
        "comment": MockAiFeedbackGenerator::comment_for(Sport::Swimming),
        "created_at": "2026-10-26T10:00:00Z",
    });

    // 生徒（getVideoReview）: 添削は未返却で、一次フィードバックは別の項目で読める
    let (status, body) = send(&router, "GET", &review_path, Some(&student), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "in_progress");
    assert_eq!(body["comment"], Value::Null);
    assert_eq!(body["ai_feedback"], expected);

    // コーチ（listMyCoachVideos）: 添削待ちの動画に一次フィードバックが載る
    let (status, list) = send(&router, "GET", "/coaches/me/videos", Some(&coach), None).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list["videos"][0]["video_id"], video_id.as_str());
    assert_eq!(list["videos"][0]["ai_feedback"], expected);

    // 翌日にコーチが返却しても、一次フィードバックは上書きされない
    clock.set(oct26_10() + Duration::from_secs(86_400));
    let (status, returned) = send(
        &router,
        "POST",
        &review_path,
        Some(&coach),
        Some(json!({ "comment": "キックを膝から打たない" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{returned}");
    assert_eq!(returned["comment"], "キックを膝から打たない");
    assert_eq!(returned["ai_feedback"], expected);

    let (_, body) = send(&router, "GET", &review_path, Some(&student), None).await;
    assert_eq!(body["status"], "returned");
    assert_eq!(body["comment"], "キックを膝から打たない");
    assert_eq!(body["ai_feedback"], expected);
    let (_, list) = send(&router, "GET", "/coaches/me/videos", Some(&coach), None).await;
    assert_eq!(list["videos"][0]["review_status"], "returned");
    assert_eq!(list["videos"][0]["ai_feedback"], expected);
}
