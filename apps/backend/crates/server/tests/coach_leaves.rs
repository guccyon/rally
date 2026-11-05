//! 本部がコーチをお休みにする（admin-web の `getCoachLeavePreview`・`startCoachLeave`）ことを、保存先を Postgres にした組み立て
//! （`server::router_with_postgres`）で通す。ルートが組み立てに入っていること、期間の予約と予約のない枠を確かめてから
//! お休みにすると、予約が本部のキャンセルになって本部のキャンセルされた予約の一覧（`listCancelledBookings`）に
//! `cancelled_by: headquarters` で出ること、生徒のお知らせ（student-app の `listMyNotifications`）にコーチの表示名・予約の
//! 日時・本部の一言の本文が届くこと（境界間イベントの購読を通して）、枠が生徒の検索（`searchSlots`）に出なくなること、
//! 同じ期間の 2 回目は 409 になることを確かめる。契約の形・失敗の応答の細かい検査は interface の `tests/coach_leaves.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test coach_leaves -- --include-ignored` で走らせる。

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

/// 2026-11-05T03:00:00Z（日本時間の 11 月 5 日 12 時）
const NOW: Duration = Duration::from_secs(1_793_847_600);

fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + NOW
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
    let sports = if role == Role::Headquarters {
        vec![]
    } else {
        vec![Sport::Tennis]
    };
    register_with_sports(pool, email, role, sports).await
}

/// 提供スポーツ（生徒ならスポーツ）`sports` のアカウントを登録して ID とトークンを返す。
async fn register_with_sports(
    pool: &PgPool,
    email: &str,
    role: Role,
    sports: Vec<Sport>,
) -> (AccountId, String) {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        SystemClock,
    )
    .execute(RegisterAccountCommand {
        email: email.to_string(),
        display_name: email.split('@').next().unwrap().to_string(),
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
    (account.id(), token)
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_puts_a_coach_on_leave_and_the_student_is_notified(pool: PgPool) {
    let (coach, coach_token) = register(&pool, "yamada@example.com", Role::Coach).await;
    let (_, student_token) = register(&pool, "sato@example.com", Role::Student).await;
    let (_, hq_token) = register(&pool, "hq@example.com", Role::Headquarters).await;
    let clock = ManualClock::new(now());
    let router = server::router_with_postgres(pool, clock, RecordingAnalyticsSink::default());
    let coach_id = account_id_uuid::format(coach);

    // 11/10 19:00（日本時間）の枠を予約し、11/13 19:00 の枠は予約しない
    let mut slot_ids = Vec::new();
    for (starts_at, ends_at) in [
        ("2026-11-10T10:00:00Z", "2026-11-10T10:30:00Z"),
        ("2026-11-13T10:00:00Z", "2026-11-13T10:30:00Z"),
    ] {
        let (status, slot) = send(
            &router,
            post_json(
                "/slots",
                &coach_token,
                json!({ "sport": "tennis", "starts_at": starts_at, "ends_at": ends_at }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{slot}");
        slot_ids.push(slot["slot_id"].clone());
    }
    let (status, booking) = send(
        &router,
        post_json(
            "/bookings",
            &student_token,
            json!({ "slot_id": slot_ids[0], "is_trial": true }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");
    let booking_id = booking["booking_id"].clone();

    let (status, preview) = send(
        &router,
        get(
            &format!("/coaches/{coach_id}/leave-preview?from=2026-11-10&to=2026-11-16"),
            &hq_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["bookings"][0]["booking_id"], booking_id);
    assert_eq!(preview["open_slots"][0]["slot_id"], slot_ids[1]);
    assert_eq!(preview["slot_count"], 2);

    let request = json!({
        "from": "2026-11-10",
        "to": "2026-11-16",
        "note": "ご迷惑をおかけします。",
        "expected_booking_ids": [booking_id],
    });
    let (status, result) = send(
        &router,
        post_json(
            &format!("/coaches/{coach_id}/leaves"),
            &hq_token,
            request.clone(),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    assert_eq!(result["withdrawn_slot_count"], 2);
    assert_eq!(result["notified_student_count"], 1);

    let (status, cancelled) = send(
        &router,
        get(
            &format!("/bookings/cancelled?coach_id={coach_id}&from=2026-11-05&to=2026-11-05"),
            &hq_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cancelled}");
    assert_eq!(cancelled["bookings"][0]["booking_id"], booking_id);
    assert_eq!(cancelled["bookings"][0]["cancelled_by"], "headquarters");

    let (status, notifications) = send(&router, get("/notifications/me", &student_token)).await;
    assert_eq!(status, StatusCode::OK, "{notifications}");
    assert_eq!(
        notifications["notifications"][0]["kind"],
        "booking_confirmed"
    );
    assert_eq!(
        notifications["notifications"][0]["body"],
        "コーチのお休みで予約をキャンセルしました\nyamada コーチのお休みのため、11/10（火）19:00〜19:30 の予約を本部でキャンセルしました。今月のビデオ通話の枠は使いません。ほかの枠やコーチを探して予約できます。\nご迷惑をおかけします。"
    );

    let (status, found) = send(&router, get("/slots?sport=tennis", &student_token)).await;
    assert_eq!(status, StatusCode::OK, "{found}");
    assert!(
        found["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|slot| !slot_ids.contains(&slot["slot_id"])),
        "取り下げた枠は検索に出ない: {found}"
    );

    let (status, again) = send(
        &router,
        post_json(&format!("/coaches/{coach_id}/leaves"), &hq_token, request),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{again}");
    assert_eq!(again["code"], "coach_leave_overlaps");
}
