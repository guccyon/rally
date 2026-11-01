//! 本部が予約 1 件の詳細を見る（admin-web の `getBooking`）ことを、保存先を Postgres にした組み立て
//! （`server::router_with_postgres`）で通す。ルートが組み立てに入っていること、Postgres に保存した予約・予約した時点の
//! レッスンメニュー・枠の開始と終わり・確定とキャンセルの時刻から状態の移り変わりを返し、コーチと生徒の表示名をアカウントの
//! 保存先から添えること、ない予約は 404、本部以外は 403 になることを確かめる。
//! 契約の形の細かい検査は interface の `tests/headquarters_booking_details.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test headquarters_booking_details -- --include-ignored` で走らせる。

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

/// 2026-10-26T00:00:00Z（日本時間の 10 月 26 日 9 時）
const NOW: Duration = Duration::from_secs(1_792_972_800);
const HOUR: Duration = Duration::from_secs(3600);

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

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_reads_a_booking_and_its_status_changes_from_postgres(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;
    let (_, hq_token) = register(&pool, "hq@example.com", Role::Headquarters).await;
    let clock = ManualClock::new(SystemTime::UNIX_EPOCH + NOW);
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    let (status, menu) = send(
        &router,
        post_json(
            "/coaches/me/lesson-menus",
            &tanaka_token,
            json!({
                "sport": "tennis",
                "lesson_type": "live_lesson",
                "name": "フォア改善",
                "duration_minutes": 60,
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{menu}");
    let (status, slot) = send(
        &router,
        post_json(
            "/slots",
            &tanaka_token,
            json!({
                "sport": "tennis",
                "starts_at": "2026-10-28T03:00:00Z",
                "ends_at": "2026-10-28T04:00:00Z",
                "lesson_menu_id": menu["lesson_menu_id"],
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
            json!({ "slot_id": slot["slot_id"], "is_trial": true }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");
    let path = format!("/bookings/{}", booking["booking_id"].as_str().unwrap());

    // 作ったばかりの予約は、状態の移り変わりが空
    let (status, body) = send(&router, get(&path, &hq_token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["booking"], booking);
    assert_eq!(body["status_changes"], json!([]));

    // 1 時間後に確定し、さらに 1 時間後にキャンセルする
    for (offset, action) in [(HOUR, "confirm"), (2 * HOUR, "cancel")] {
        clock.set(SystemTime::UNIX_EPOCH + NOW + offset);
        let (status, body) = send(
            &router,
            post_json(&format!("{path}/{action}"), &taro_token, json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{action}: {body}");
    }

    let (status, body) = send(&router, get(&path, &hq_token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["booking"]["booking_id"], booking["booking_id"]);
    assert_eq!(body["booking"]["status"], "cancelled");
    assert_eq!(body["booking"]["is_trial"], true);
    assert_eq!(body["booking"]["created_at"], "2026-10-26T00:00:00Z");
    assert_eq!(body["booking"]["coach_id"], account_id_uuid::format(tanaka));
    assert_eq!(body["booking"]["student_id"], account_id_uuid::format(taro));
    assert_eq!(body["booking"]["lesson_menu"]["name"], "フォア改善");
    assert_eq!(body["booking"]["lesson_menu"]["duration_minutes"], 60);
    assert_eq!(body["slot"]["slot_id"], slot["slot_id"]);
    assert_eq!(body["slot"]["starts_at"], "2026-10-28T03:00:00Z");
    assert_eq!(body["slot"]["ends_at"], "2026-10-28T04:00:00Z");
    assert_eq!(
        body["status_changes"],
        json!([
            { "status": "confirmed", "changed_at": "2026-10-26T01:00:00Z" },
            { "status": "cancelled", "changed_at": "2026-10-26T02:00:00Z" },
        ])
    );
    assert_eq!(body["coach_display_name"], "tanaka");
    assert_eq!(body["student_display_name"], "taro");

    // ない予約は 404
    let (status, body) = send(
        &router,
        get("/bookings/00000000-0000-0000-0000-000000000063", &hq_token),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["code"], "not_found");

    // 本部以外は 403（予約した生徒本人と枠のコーチでも）
    for token in [&taro_token, &tanaka_token] {
        let (status, body) = send(&router, get(&path, token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
}
