//! 本部が生徒 1 人の利用の様子を見る（admin-web の `GET /students/{student_id}/activity`）ことを、保存先を Postgres にした
//! 組み立て（`server::router_with_postgres`）で通す。生徒が student-app の操作で予約し・キャンセルし・動画添削を受けて
//! 完了を記録したことが、これからの予約・キャンセルの件数・直近の受けたレッスンに出ること、何も保存しないこと、
//! コーチ・生徒は 403、生徒でない ID は 404 であることを確かめる。契約の形の細かい検査は interface の
//! `tests/student_activity.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test student_activity -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    identity::{
        RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer, account_id_uuid,
    },
    in_memory::{ManualClock, RecordingAnalyticsSink},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{catalog::Sport, identity::Role};
use infra::{MockTokenService, PgAccountRepository};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

/// 2026-10-05T00:00:00Z から `secs` 秒後（Postgres の時刻はマイクロ秒までなので、秒単位にする）。
fn at(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_791_158_400 + secs)
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

/// `token` で `path` に POST し、`expected` の status であることを確かめて本文を返す。
async fn post_ok(
    router: &Router,
    path: &str,
    token: &str,
    body: Value,
    expected: StatusCode,
) -> Value {
    let (status, body) = send(router, "POST", path, Some(token), Some(body)).await;
    assert_eq!(status, expected, "{path}: {body}");
    body
}

/// 役割 `role` のアカウントをユースケースを直に呼んで登録し（契約にコーチ・本部の登録の操作がない）、ID とトークンを返す。
async fn register(
    pool: &PgPool,
    email: &str,
    display_name: &str,
    role: Role,
    sports: Vec<Sport>,
) -> (String, String) {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        ManualClock::new(at(0)),
    )
    .execute(RegisterAccountCommand {
        email: email.to_string(),
        display_name: display_name.to_string(),
        role,
        sports,
    })
    .await
    .unwrap();
    let token = MockTokenService
        .issue(TokenClaims {
            account_id: account.id(),
            role,
        })
        .into_string();
    (account_id_uuid::format(account.id()), token)
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_sees_upcoming_bookings_cancellations_and_lessons_of_a_student(pool: PgPool) {
    let (coach_id, coach) = register(
        &pool,
        "coach@example.com",
        "たなか",
        Role::Coach,
        vec![Sport::Tennis],
    )
    .await;
    let (headquarters_id, headquarters) =
        register(&pool, "hq@example.com", "本部", Role::Headquarters, vec![]).await;
    let clock = ManualClock::new(at(60));
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    let (status, account) = send(
        &router,
        "POST",
        "/accounts",
        None,
        Some(json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "tennis" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{account}");
    let student_id = account["student"]["student_id"]
        .as_str()
        .unwrap()
        .to_string();
    let student = account["access_token"].as_str().unwrap().to_string();

    // 2 つの枠を予約し、遅いほうをキャンセルする
    let mut booking_ids = Vec::new();
    for day in ["2099-01-01", "2099-01-02"] {
        let slot = post_ok(
            &router,
            "/slots",
            &coach,
            json!({
                "sport": "tennis",
                "starts_at": format!("{day}T10:00:00Z"),
                "ends_at": format!("{day}T11:00:00Z"),
            }),
            StatusCode::CREATED,
        )
        .await;
        let booking = post_ok(
            &router,
            "/bookings",
            &student,
            json!({ "slot_id": slot["slot_id"], "is_trial": false }),
            StatusCode::CREATED,
        )
        .await;
        booking_ids.push(booking["booking_id"].as_str().unwrap().to_string());
    }
    clock.set(at(120));
    post_ok(
        &router,
        &format!("/bookings/{}/cancel", booking_ids[1]),
        &student,
        json!({}),
        StatusCode::OK,
    )
    .await;

    // 動画を送り、添削が返されてから完了を記録する
    let video = post_ok(
        &router,
        "/videos",
        &student,
        json!({ "coach_id": coach_id, "sport": "tennis", "content_type": "video/mp4" }),
        StatusCode::CREATED,
    )
    .await;
    let video_id = video["video_id"].as_str().unwrap();
    clock.set(at(180));
    post_ok(
        &router,
        &format!("/videos/{video_id}/review"),
        &coach,
        json!({ "comment": "フォロースルーを大きく" }),
        StatusCode::OK,
    )
    .await;
    clock.set(at(240));
    let lesson = post_ok(
        &router,
        "/lessons/completions",
        &student,
        json!({ "lesson_type": "async_lesson", "video_id": video_id }),
        StatusCode::CREATED,
    )
    .await;

    let path = format!("/students/{student_id}/activity");
    let (status, body) = send(&router, "GET", &path, Some(&headquarters), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["student_id"], student_id);
    assert_eq!(body["display_name"], "たろう");
    assert_eq!(body["cancelled_booking_count"], 1);
    let upcoming = body["upcoming_bookings"].as_array().unwrap();
    assert_eq!(upcoming.len(), 1, "キャンセルした予約は載せない: {body}");
    assert_eq!(upcoming[0]["booking"]["booking_id"], booking_ids[0]);
    assert_eq!(upcoming[0]["booking"]["status"], "pending");
    assert_eq!(upcoming[0]["coach_display_name"], "たなか");
    assert_eq!(
        body["recent_lessons"],
        json!([{
            "lesson_id": lesson["lesson_id"],
            "coach_id": coach_id,
            "coach_display_name": "たなか",
            "lesson_type": "async_lesson",
            "completed_at": "2026-10-05T00:04:00Z",
        }])
    );

    // 照会だけで何も変えない（2 回読んでも同じ）
    let (_, again) = send(&router, "GET", &path, Some(&headquarters), None).await;
    assert_eq!(again, body);

    for token in [&coach, &student] {
        let (status, body) = send(&router, "GET", &path, Some(token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
    for id in [&coach_id, &headquarters_id] {
        let (status, body) = send(
            &router,
            "GET",
            &format!("/students/{id}/activity"),
            Some(&headquarters),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "生徒でない: {body}");
    }
}
