//! 本部が期間とコーチで通話セッションの記録を見る（admin-web の `listLiveLessonSessions`）ことを、保存先を Postgres に
//! した組み立て（`server::router_with_postgres`）で通す。ルートが組み立てに入っていて同じパスの開始（`POST`）と共存する
//! こと、API で開始・終了した Postgres のセッションを日付（日本時間）の期間で開始の新しい順に返し、終了の時刻と長さ（分）・
//! コーチと生徒の表示名を添えること、`coach_id` で絞れること、期間の不正が 422・本部以外が 403 になることを確かめる。
//! 契約の形・期間の細かい検査は interface の `tests/headquarters_live_lesson_sessions.rs`、期間の読み取りは infra の
//! `tests/pg_live_lesson_session_repository.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test headquarters_live_lesson_sessions -- --include-ignored` で走らせる。

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

/// 2026-11-01T10:00:00Z（最初のスロットの開始）
fn nov1_10() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200) + 10 * HOUR
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

/// コーチ `coach` が `starts_at` から `ends_at` までの枠を公開し、生徒 `student` が予約して確定した予約の ID。
async fn confirmed_booking(
    router: &Router,
    coach: &str,
    student: &str,
    (starts_at, ends_at): (&str, &str),
) -> Value {
    let (status, slot) = send(
        router,
        post_json(
            "/slots",
            coach,
            json!({ "sport": "tennis", "starts_at": starts_at, "ends_at": ends_at }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{slot}");
    let (status, booking) = send(
        router,
        post_json(
            "/bookings",
            student,
            json!({ "slot_id": slot["slot_id"], "is_trial": true }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");
    let booking_id = booking["booking_id"].clone();
    let (status, body) = send(
        router,
        post_json(
            &format!("/bookings/{}/confirm", booking_id.as_str().unwrap()),
            student,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    booking_id
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_reads_session_records_from_postgres(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (suzuki, suzuki_token) = register(&pool, "suzuki@example.com", Role::Coach).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;
    let (hanako, hanako_token) = register(&pool, "hanako@example.com", Role::Student).await;
    let (_, jiro_token) = register(&pool, "jiro@example.com", Role::Student).await;
    let (_, hq_token) = register(&pool, "hq@example.com", Role::Headquarters).await;
    let clock = ManualClock::new(nov1_10() - 24 * HOUR);
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    let left_open = confirmed_booking(
        &router,
        &tanaka_token,
        &taro_token,
        ("2026-11-01T10:00:00Z", "2026-11-01T11:00:00Z"),
    )
    .await;
    let ended = confirmed_booking(
        &router,
        &tanaka_token,
        &hanako_token,
        ("2026-11-01T11:00:00Z", "2026-11-01T12:00:00Z"),
    )
    .await;
    // 2026-11-01T15:00:00Z は日本時間の 11 月 2 日 0 時
    let next_day = confirmed_booking(
        &router,
        &suzuki_token,
        &jiro_token,
        ("2026-11-01T15:00:00Z", "2026-11-01T16:00:00Z"),
    )
    .await;

    // 10 時にたろうの通話を始めたまま終了しない。11 時のはなこの通話は 11 時 45 分 30 秒に終了する
    clock.set(nov1_10());
    let (status, open_session) = send(
        &router,
        post_json(
            "/live-lesson-sessions",
            &tanaka_token,
            json!({ "booking_id": left_open }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{open_session}");
    clock.set(nov1_10() + HOUR);
    let (status, ended_session) = send(
        &router,
        post_json(
            "/live-lesson-sessions",
            &hanako_token,
            json!({ "booking_id": ended }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{ended_session}");
    clock.set(nov1_10() + HOUR + Duration::from_secs(45 * 60 + 30));
    let (status, body) = send(
        &router,
        post_json(
            &format!(
                "/live-lesson-sessions/{}/end",
                ended_session["session_id"].as_str().unwrap()
            ),
            &hanako_token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    clock.set(nov1_10() + 5 * HOUR);
    let (status, next_day_session) = send(
        &router,
        post_json(
            "/live-lesson-sessions",
            &suzuki_token,
            json!({ "booking_id": next_day }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{next_day_session}");

    // 日本時間の 11 月 1 日だけ。新しい順に、終了したはなこの通話・通話中のたろうの通話
    let (status, body) = send(
        &router,
        get(
            "/live-lesson-sessions?from=2026-11-01&to=2026-11-01",
            &hq_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "from": "2026-11-01",
            "to": "2026-11-01",
            "sessions": [
                {
                    "session_id": ended_session["session_id"],
                    "booking_id": ended,
                    "coach_id": account_id_uuid::format(tanaka),
                    "coach_display_name": "tanaka",
                    "student_id": account_id_uuid::format(hanako),
                    "student_display_name": "hanako",
                    "started_at": "2026-11-01T11:00:00Z",
                    "ended_at": "2026-11-01T11:45:30Z",
                    "duration_minutes": 45,
                },
                {
                    "session_id": open_session["session_id"],
                    "booking_id": left_open,
                    "coach_id": account_id_uuid::format(tanaka),
                    "coach_display_name": "tanaka",
                    "student_id": account_id_uuid::format(taro),
                    "student_display_name": "taro",
                    "started_at": "2026-11-01T10:00:00Z",
                    "ended_at": null,
                    "duration_minutes": null,
                },
            ],
        })
    );

    // 2 日にまたがれば翌日のすずきの通話が先頭。コーチで絞れる
    let (status, body) = send(
        &router,
        get(
            "/live-lesson-sessions?from=2026-11-01&to=2026-11-02",
            &hq_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ids: Vec<&Value> = body["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| &s["session_id"])
        .collect();
    assert_eq!(
        ids,
        [
            &next_day_session["session_id"],
            &ended_session["session_id"],
            &open_session["session_id"],
        ]
    );
    let (status, body) = send(
        &router,
        get(
            &format!(
                "/live-lesson-sessions?from=2026-11-01&to=2026-11-02&coach_id={}",
                account_id_uuid::format(suzuki)
            ),
            &hq_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["sessions"].as_array().unwrap().len(), 1, "{body}");
    assert_eq!(
        body["sessions"][0]["session_id"],
        next_day_session["session_id"]
    );
    assert_eq!(body["sessions"][0]["coach_display_name"], "suzuki");

    // 期間の不正は 422
    let (status, body) = send(
        &router,
        get(
            "/live-lesson-sessions?from=2026-11-01&to=2026-12-02",
            &hq_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");

    // 本部だけ
    for token in [&tanaka_token, &taro_token] {
        let (status, body) = send(
            &router,
            get("/live-lesson-sessions?from=2026-11-01&to=2026-11-01", token),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
}
