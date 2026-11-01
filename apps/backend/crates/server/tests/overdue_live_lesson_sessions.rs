//! 本部が終わっていないまま時間の過ぎた通話セッションの一覧を見る（admin-web の `listOverdueLiveLessonSessions`）ことを、
//! 保存先を Postgres にした組み立て（`server::router_with_postgres`）で通す。ルートが組み立てに入っていること、API で
//! 開始した Postgres のセッションのうち終了していないものだけを、開始から既定の 3 時間（`older_than_hours` で変えられる）
//! 以上たったときに返し、コーチと生徒の表示名をアカウントの保存先から添えること、本部以外は 403 になることを確かめる。
//! 契約の形・時間の範囲の細かい検査は interface の `tests/overdue_live_lesson_sessions.rs`、通話中のセッションの読み取りは
//! infra の `tests/pg_live_lesson_session_repository.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test overdue_live_lesson_sessions -- --include-ignored` で走らせる。

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
async fn headquarters_reads_overdue_sessions_from_postgres(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;
    let (_, hanako_token) = register(&pool, "hanako@example.com", Role::Student).await;
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

    // 10 時にたろうの通話を始めたまま終了しない。11 時のはなこの通話は 11 時 45 分に終了する
    clock.set(nov1_10());
    let (status, session) = send(
        &router,
        post_json(
            "/live-lesson-sessions",
            &tanaka_token,
            json!({ "booking_id": left_open }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{session}");
    let session_id = session["session_id"].clone();
    clock.set(nov1_10() + HOUR);
    let (status, other) = send(
        &router,
        post_json(
            "/live-lesson-sessions",
            &hanako_token,
            json!({ "booking_id": ended }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{other}");
    clock.set(nov1_10() + HOUR + Duration::from_secs(45 * 60));
    let (status, body) = send(
        &router,
        post_json(
            &format!(
                "/live-lesson-sessions/{}/end",
                other["session_id"].as_str().unwrap()
            ),
            &hanako_token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // 開始から 2 時間 59 分では、まだ出ない
    clock.set(nov1_10() + 3 * HOUR - Duration::from_secs(60));
    let (status, body) = send(&router, get("/live-lesson-sessions/overdue", &hq_token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "older_than_hours": 3, "sessions": [] }));

    // 開始から 3 時間 30 分。終了したはなこの通話は出ない
    clock.set(nov1_10() + 3 * HOUR + Duration::from_secs(30 * 60));
    let (status, body) = send(&router, get("/live-lesson-sessions/overdue", &hq_token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "older_than_hours": 3,
            "sessions": [{
                "session_id": session_id,
                "booking_id": left_open,
                "coach_id": account_id_uuid::format(tanaka),
                "coach_display_name": "tanaka",
                "student_id": account_id_uuid::format(taro),
                "student_display_name": "taro",
                "started_at": "2026-11-01T10:00:00Z",
                "elapsed_sec": 3 * 3600 + 30 * 60,
            }],
        })
    );

    // 時間を変えられる
    let (status, body) = send(
        &router,
        get(
            "/live-lesson-sessions/overdue?older_than_hours=4",
            &hq_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "older_than_hours": 4, "sessions": [] }));

    // 本部だけ
    for token in [&tanaka_token, &taro_token] {
        let (status, body) = send(&router, get("/live-lesson-sessions/overdue", token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
}
