//! 本部が予約の一覧を日付で見る（admin-web の `listBookings`）ことを、保存先を Postgres にした組み立て
//! （`server::router_with_postgres`）で通す。ルートが組み立てに入っていること、Postgres の予約を日本時間の日付で絞って
//! 開始の古い順に読み、コーチと生徒の表示名をアカウントの保存先から添えること、`coach_id` でそのコーチの予約だけに絞れること、
//! `status` で Postgres に保存した状態（確定待ち・確定済み・キャンセル済み）の予約だけに絞れること、本部以外は 403 になることを
//! 確かめる。
//! 契約の形・期間の規則の細かい検査は interface の `tests/headquarters_bookings.rs`、期間に開始する予約の読み取りは
//! infra の `tests/pg_booking_repository.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test headquarters_bookings -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    clock::SystemClock,
    identity::{
        RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer, account_id_uuid,
    },
    in_memory::{FixedClock, RecordingAnalyticsSink},
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
async fn headquarters_reads_bookings_by_date_from_postgres(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (_, ito_token) = register(&pool, "ito@example.com", Role::Coach).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;
    let (_, hq_token) = register(&pool, "hq@example.com", Role::Headquarters).await;
    let router = server::router_with_postgres(
        pool,
        FixedClock(SystemTime::UNIX_EPOCH + NOW),
        RecordingAnalyticsSink::default(),
    );

    // 今日（日本時間の 10 月 26 日）から 7 日（11 月 1 日まで）に 2 つ、その後（11 月 2 日 0 時）に 1 つ
    let mut slots = Vec::new();
    for (coach, starts_at, ends_at, is_trial) in [
        (
            &ito_token,
            "2026-10-27T15:00:00Z",
            "2026-10-27T16:00:00Z",
            false,
        ),
        (
            &tanaka_token,
            "2026-10-26T03:00:00Z",
            "2026-10-26T04:00:00Z",
            true,
        ),
        (
            &tanaka_token,
            "2026-11-01T15:00:00Z",
            "2026-11-01T16:00:00Z",
            false,
        ),
    ] {
        let (status, slot) = send(
            &router,
            post_json(
                "/slots",
                coach,
                json!({ "sport": "tennis", "starts_at": starts_at, "ends_at": ends_at }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{slot}");
        // 体験予約はその生徒の初回の予約だけなので、先に体験予約の枠を予約する
        slots.push((is_trial, slot["slot_id"].clone()));
    }
    slots.sort_by_key(|(is_trial, _)| !is_trial);
    let mut booked = Vec::new();
    for (is_trial, slot_id) in slots {
        let (status, booking) = send(
            &router,
            post_json(
                "/bookings",
                &taro_token,
                json!({ "slot_id": slot_id, "is_trial": is_trial }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{booking}");
        booked.push(booking);
    }

    let (status, body) = send(&router, get("/bookings", &hq_token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["from"], "2026-10-26");
    assert_eq!(body["to"], "2026-11-01");
    let rows = body["bookings"].as_array().unwrap();
    assert_eq!(
        rows.len(),
        2,
        "11 月 2 日 0 時（日本時間）の予約は入らない: {body}"
    );
    assert_eq!(rows[0]["booking"], booked[0]);
    assert_eq!(rows[0]["booking"]["is_trial"], true);
    assert_eq!(
        rows[0]["booking"]["coach_id"],
        account_id_uuid::format(tanaka)
    );
    assert_eq!(rows[0]["coach_display_name"], "tanaka");
    assert_eq!(
        rows[0]["booking"]["student_id"],
        account_id_uuid::format(taro)
    );
    assert_eq!(rows[0]["student_display_name"], "taro");
    assert_eq!(rows[1]["booking"]["starts_at"], "2026-10-27T15:00:00Z");
    assert_eq!(rows[1]["coach_display_name"], "ito");

    let (status, body) = send(
        &router,
        get("/bookings?from=2026-11-02&to=2026-11-02", &hq_token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["bookings"].as_array().unwrap().len(), 1, "{body}");
    assert_eq!(body["bookings"][0]["booking"], booked[2]);

    // コーチで絞る。期間と組み合わせられ、いないコーチは空の一覧
    let (status, body) = send(
        &router,
        get(
            &format!(
                "/bookings?from=2026-10-26&to=2026-11-02&coach_id={}",
                account_id_uuid::format(tanaka)
            ),
            &hq_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["bookings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| &row["booking"])
            .collect::<Vec<_>>(),
        [&booked[0], &booked[2]],
        "{body}"
    );
    let (status, body) = send(
        &router,
        get(
            &format!(
                "/bookings?coach_id={}",
                account_id_uuid::format(AccountId::from_u128(999))
            ),
            &hq_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["bookings"], json!([]));
    let (status, body) = send(&router, get("/bookings?coach_id=tanaka", &hq_token)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");

    // 状態で絞る。1 件目（10 月 26 日）を確定し、2 件目（10 月 27 日）をキャンセルして、3 件目（11 月 2 日）は確定待ちのまま
    for (booking, action) in [(&booked[0], "confirm"), (&booked[1], "cancel")] {
        let (status, body) = send(
            &router,
            post_json(
                &format!(
                    "/bookings/{}/{action}",
                    booking["booking_id"].as_str().unwrap()
                ),
                &taro_token,
                json!({}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{action}: {body}");
    }
    let statuses_of = |body: &Value| -> Vec<(Value, Value)> {
        body["bookings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                (
                    row["booking"]["booking_id"].clone(),
                    row["booking"]["status"].clone(),
                )
            })
            .collect()
    };
    let id = |i: usize| booked[i]["booking_id"].clone();
    for (query, expected) in [
        ("&status=confirmed", vec![(id(0), json!("confirmed"))]),
        ("&status=cancelled", vec![(id(1), json!("cancelled"))]),
        ("&status=pending", vec![(id(2), json!("pending"))]),
        (
            "&status=pending&status=cancelled",
            vec![(id(1), json!("cancelled")), (id(2), json!("pending"))],
        ),
        (
            "",
            vec![
                (id(0), json!("confirmed")),
                (id(1), json!("cancelled")),
                (id(2), json!("pending")),
            ],
        ),
    ] {
        let (status, body) = send(
            &router,
            get(
                &format!("/bookings?from=2026-10-26&to=2026-11-02{query}"),
                &hq_token,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(statuses_of(&body), expected, "{query}: {body}");
    }
    let (status, body) = send(&router, get("/bookings?status=done", &hq_token)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");

    for token in [&tanaka_token, &taro_token] {
        let (status, _) = send(&router, get("/bookings", token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
}
