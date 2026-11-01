//! 本部が通話セッションを本部の判断で終わらせる（admin-web の `endLiveLessonSessionByHeadquarters`）ことを、保存先を
//! Postgres にした組み立て（`server::router_with_postgres`）で通す。ルートが組み立てに入っていること、API で開始したまま
//! 残った Postgres のセッションを操作の時刻で終わらせ、本部のアカウントを残すこと、終わらせた後は終わっていないセッションの
//! 一覧に出ず期間の記録に終了の時刻つきで載ること、参加者の終了と 2 回目が 409 になること、月額の生徒のビデオ通話の暦月の枠
//! （`getMyEntitlement` の `live_lesson_quota.used`）が変わらないこと、本部以外は 403 になることを確かめる。
//! 契約の形の細かい検査は interface の `tests/headquarters_live_lesson_session_ends.rs`、保存の往復は infra の
//! `tests/pg_live_lesson_session_repository.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test headquarters_live_lesson_session_ends -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    clock::SystemClock,
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
use domain::{
    catalog::Sport,
    identity::{AccountId, Role},
};
use infra::{MockTokenService, PgAccountRepository};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

const HOUR: Duration = Duration::from_secs(3600);

/// 2026-11-10T10:00:00Z（スロットの開始。前の日の予約と同じ暦月にする）
fn nov10_10() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200) + (9 * 24 + 10) * HOUR
}

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn get(path: &str, token: &str) -> Request<Body> {
    Request::get(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

fn post_json(path: &str, token: &str, body: Value) -> Request<Body> {
    Request::post(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// アカウントを登録して ID とトークンを返す。契約にコーチ・本部の登録の操作がないので、ユースケースを直に呼ぶ。
async fn register(pool: &PgPool, email: &str, role: Role) -> (AccountId, String) {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        SystemClock,
    )
    .execute(RegisterAccountCommand {
        email: email.to_string(),
        display_name: email.split('@').next().unwrap().to_string(),
        role,
        sports: if role == Role::Headquarters {
            vec![]
        } else {
            vec![Sport::Tennis]
        },
    })
    .await
    .unwrap();
    let token = MockTokenService
        .issue(TokenClaims {
            account_id: account.id(),
            role,
        })
        .into_string();
    (account.id(), token)
}

/// 生徒 `student` のビデオ通話の暦月の枠を使った回数。
async fn used_live_lessons(router: &Router, student: &str) -> Value {
    let (status, entitlement) = send(router, get("/entitlements/me", student)).await;
    assert_eq!(status, StatusCode::OK, "{entitlement}");
    entitlement["live_lesson_quota"]["used"].clone()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_ends_a_postgres_session_left_in_progress(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;
    let (hq, hq_token) = register(&pool, "hq@example.com", Role::Headquarters).await;
    let clock = ManualClock::new(nov10_10() - 24 * HOUR);
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    // たろうは月額契約で、10 時の枠を予約して確定する（枠を 1 回使う）
    let (status, body) = send(
        &router,
        post_json(
            "/subscriptions",
            &taro_token,
            json!({ "sport": "tennis", "payment_method_token": "tok_mock" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, slot) = send(
        &router,
        post_json(
            "/slots",
            &tanaka_token,
            json!({
                "sport": "tennis",
                "starts_at": "2026-11-10T10:00:00Z",
                "ends_at": "2026-11-10T11:00:00Z",
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{slot}");
    let (status, booking) = send(
        &router,
        post_json(
            "/bookings",
            &taro_token,
            json!({ "slot_id": slot["slot_id"], "is_trial": false }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");
    let booking_id = booking["booking_id"].clone();
    let (status, body) = send(
        &router,
        post_json(
            &format!("/bookings/{}/confirm", booking_id.as_str().unwrap()),
            &taro_token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let used = used_live_lessons(&router, &taro_token).await;
    assert_eq!(used, 1);

    // 10 時に通話を始めたまま終了しない
    clock.set(nov10_10());
    let (status, session) = send(
        &router,
        post_json(
            "/live-lesson-sessions",
            &tanaka_token,
            json!({ "booking_id": booking_id }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{session}");
    let session_id = session["session_id"].as_str().unwrap().to_string();
    let end_path = format!("/live-lesson-sessions/{session_id}/headquarters-end");

    // 本部以外は 403
    clock.set(nov10_10() + 5 * HOUR);
    for token in [&tanaka_token, &taro_token] {
        let (status, body) = send(&router, post_json(&end_path, token, json!({}))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
    let (status, body) = send(&router, get("/live-lesson-sessions/overdue", &hq_token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["sessions"].as_array().unwrap().len(), 1, "{body}");

    // 15 時に本部が終わらせる
    let (status, body) = send(&router, post_json(&end_path, &hq_token, json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "session_id": session_id,
            "booking_id": booking_id,
            "coach_id": account_id_uuid::format(tanaka),
            "student_id": account_id_uuid::format(taro),
            "started_at": "2026-11-10T10:00:00Z",
            "ended_at": "2026-11-10T15:00:00Z",
            "ended_by": account_id_uuid::format(hq),
        })
    );

    // 終わっていないセッションの一覧に出ず、期間の記録には終了の時刻と長さつきで載る
    let (status, body) = send(&router, get("/live-lesson-sessions/overdue", &hq_token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["sessions"], json!([]));
    let (status, body) = send(
        &router,
        get(
            "/live-lesson-sessions?from=2026-11-10&to=2026-11-10",
            &hq_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["sessions"][0]["session_id"], session_id);
    assert_eq!(body["sessions"][0]["ended_at"], "2026-11-10T15:00:00Z");
    assert_eq!(body["sessions"][0]["duration_minutes"], 300);

    // 2 回目も、参加者の終了も 409
    let (status, body) = send(&router, post_json(&end_path, &hq_token, json!({}))).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "invalid_session_state");
    let (status, body) = send(
        &router,
        post_json(
            &format!("/live-lesson-sessions/{session_id}/end"),
            &tanaka_token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "invalid_session_state");

    // ないセッションは 404
    let (status, body) = send(
        &router,
        post_json(
            "/live-lesson-sessions/00000000-0000-0000-0000-000000000099/headquarters-end",
            &hq_token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");

    // ビデオ通話の暦月の枠は変わらない（枠は予約の確定とキャンセルで数える。docs/adr/0005）
    assert_eq!(used_live_lessons(&router, &taro_token).await, used);
}
