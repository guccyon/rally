//! 本部が予約の一覧を日付で見る（admin-web の `listBookings`）ことを、保存先を Postgres にした組み立て
//! （`server::router_with_postgres`）で通す。ルートが組み立てに入っていること、Postgres の予約を日本時間の日付で絞って
//! 開始の古い順に読み、コーチと生徒の表示名をアカウントの保存先から添えること、`coach_id` でそのコーチの予約だけに絞れること、
//! `status` で Postgres に保存した状態（確定待ち・確定済み・キャンセル済み）の予約だけに絞れること、`student_id` でその生徒の
//! 予約だけに絞れること、`lesson_menu_id` で Postgres に保存した予約した時点のメニューの写しがそのメニューの予約だけに
//! 絞れること、`sport` で Postgres に保存したコーチの提供スポーツがその競技の予約だけに絞れること、本部以外は 403 に
//! なることを確かめる。
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
async fn headquarters_reads_bookings_by_date_from_postgres(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (ito, ito_token) = register(&pool, "ito@example.com", Role::Coach).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;
    let (jiro, jiro_token) = register(&pool, "jiro@example.com", Role::Student).await;
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

    // 体験予約かで絞る。体験予約は 1 件目（確定済み）だけ。状態と組み合わせられる
    for (query, expected) in [
        ("&trial=true", vec![(id(0), json!("confirmed"))]),
        (
            "&trial=false",
            vec![(id(1), json!("cancelled")), (id(2), json!("pending"))],
        ),
        (
            "&trial=false&status=pending",
            vec![(id(2), json!("pending"))],
        ),
        ("&trial=true&status=pending", vec![]),
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
    let (status, body) = send(&router, get("/bookings?trial=yes", &hq_token)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");

    // 生徒で絞る。別の生徒（じろう）がいとうの枠（10 月 28 日）を体験予約する。期間・コーチ・状態・体験予約かと組み合わせられ、
    // 生徒でない ID・ない ID は空の一覧
    let (status, slot) = send(
        &router,
        post_json(
            "/slots",
            &ito_token,
            json!({ "sport": "tennis", "starts_at": "2026-10-28T03:00:00Z", "ends_at": "2026-10-28T04:00:00Z" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{slot}");
    let (status, jiro_booking) = send(
        &router,
        post_json(
            "/bookings",
            &jiro_token,
            json!({ "slot_id": slot["slot_id"], "is_trial": true }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{jiro_booking}");
    let jiro_row = (jiro_booking["booking_id"].clone(), json!("pending"));
    let taro_id = account_id_uuid::format(taro);
    let jiro_id = account_id_uuid::format(jiro);
    let ito_id = account_id_uuid::format(ito);
    for (query, expected) in [
        (
            format!("&student_id={taro_id}"),
            vec![
                (id(0), json!("confirmed")),
                (id(1), json!("cancelled")),
                (id(2), json!("pending")),
            ],
        ),
        (format!("&student_id={jiro_id}"), vec![jiro_row.clone()]),
        (
            format!("&student_id={taro_id}&coach_id={ito_id}"),
            vec![(id(1), json!("cancelled"))],
        ),
        (
            format!("&student_id={jiro_id}&status=pending&trial=true"),
            vec![jiro_row.clone()],
        ),
        (format!("&student_id={jiro_id}&status=confirmed"), vec![]),
        (
            format!("&student_id={}", account_id_uuid::format(tanaka)),
            vec![],
        ),
        (
            format!(
                "&student_id={}",
                account_id_uuid::format(AccountId::from_u128(999))
            ),
            vec![],
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
    let (status, body) = send(&router, get("/bookings?student_id=taro", &hq_token)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");

    // レッスンメニューで絞る。たなかが同期レッスンのメニューを作って枠（10 月 29 日）に結びつけ、じろうが予約する
    // （予約がメニューの写しを残す）。メニューのない予約は出ず、期間・コーチ・生徒・状態・体験予約かと組み合わせられ、
    // ないメニューは空の一覧
    let (status, menu) = send(
        &router,
        post_json(
            "/coaches/me/lesson-menus",
            &tanaka_token,
            json!({
                "sport": "tennis",
                "lesson_type": "live_lesson",
                "name": "フォア改善",
                "description": "ラリーが続かない人向け",
                "duration_minutes": 60,
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{menu}");
    let menu_id = menu["lesson_menu_id"].as_str().unwrap().to_string();
    let (status, slot) = send(
        &router,
        post_json(
            "/slots",
            &tanaka_token,
            json!({
                "sport": "tennis",
                "starts_at": "2026-10-29T03:00:00Z",
                "ends_at": "2026-10-29T04:00:00Z",
                "lesson_menu_id": menu_id,
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{slot}");
    let (status, menu_booking) = send(
        &router,
        post_json(
            "/bookings",
            &jiro_token,
            json!({ "slot_id": slot["slot_id"], "is_trial": false }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{menu_booking}");
    let menu_row = (menu_booking["booking_id"].clone(), json!("pending"));
    let tanaka_id = account_id_uuid::format(tanaka);
    for (query, expected) in [
        (format!("&lesson_menu_id={menu_id}"), vec![menu_row.clone()]),
        (
            format!("&lesson_menu_id={menu_id}&coach_id={tanaka_id}&student_id={jiro_id}"),
            vec![menu_row.clone()],
        ),
        (
            format!("&lesson_menu_id={menu_id}&status=pending&trial=false"),
            vec![menu_row.clone()],
        ),
        (
            format!("&lesson_menu_id={menu_id}&coach_id={ito_id}"),
            vec![],
        ),
        (
            format!("&lesson_menu_id={menu_id}&student_id={taro_id}"),
            vec![],
        ),
        (
            format!("&lesson_menu_id={menu_id}&status=confirmed"),
            vec![],
        ),
        (format!("&lesson_menu_id={menu_id}&trial=true"), vec![]),
        (
            "&lesson_menu_id=00000000-0000-0000-0000-0000000003e7".to_string(),
            vec![],
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
    let (_, body) = send(
        &router,
        get(&format!("/bookings?lesson_menu_id={menu_id}"), &hq_token),
    )
    .await;
    assert_eq!(
        body["bookings"][0]["booking"]["lesson_menu"]["lesson_menu_id"], menu_id,
        "{body}"
    );
    // 期間と組み合わせる（10 月 30 日からは、メニューの予約の 10 月 29 日を含まない）
    let (status, body) = send(
        &router,
        get(
            &format!("/bookings?from=2026-10-30&to=2026-11-02&lesson_menu_id={menu_id}"),
            &hq_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["bookings"], json!([]), "{body}");
    // 付けなければ今までどおりメニューを問わない
    let (_, body) = send(
        &router,
        get("/bookings?from=2026-10-26&to=2026-11-02", &hq_token),
    )
    .await;
    assert_eq!(body["bookings"].as_array().unwrap().len(), 5, "{body}");
    let (status, body) = send(&router, get("/bookings?lesson_menu_id=menu", &hq_token)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");

    for token in [&tanaka_token, &taro_token] {
        for path in ["/bookings", "/bookings?trial=true"] {
            let (status, _) = send(&router, get(path, token)).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
        }
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_narrows_bookings_by_the_sport_of_the_coach_in_postgres(pool: PgPool) {
    // たなかはテニスとゴルフ、いとうはサッカーを提供する。枠のスポーツはコーチの最初の提供スポーツ
    let (tanaka, tanaka_token) = register_with_sports(
        &pool,
        "tanaka@example.com",
        Role::Coach,
        vec![Sport::Tennis, Sport::Golf],
    )
    .await;
    let (ito, ito_token) =
        register_with_sports(&pool, "ito@example.com", Role::Coach, vec![Sport::Soccer]).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;
    let (_, jiro_token) = register(&pool, "jiro@example.com", Role::Student).await;
    let (_, hq_token) = register(&pool, "hq@example.com", Role::Headquarters).await;
    let router = server::router_with_postgres(
        pool,
        FixedClock(SystemTime::UNIX_EPOCH + NOW),
        RecordingAnalyticsSink::default(),
    );

    // 10 月 26 日にたなかの枠（テニス）、10 月 27 日にいとうの枠（サッカー）、10 月 28 日にたなかの枠（テニス）
    let mut booked = Vec::new();
    for (coach, sport, starts_at, ends_at, student) in [
        (
            &tanaka_token,
            "tennis",
            "2026-10-26T03:00:00Z",
            "2026-10-26T04:00:00Z",
            &taro_token,
        ),
        (
            &ito_token,
            "soccer",
            "2026-10-27T03:00:00Z",
            "2026-10-27T04:00:00Z",
            &taro_token,
        ),
        (
            &tanaka_token,
            "tennis",
            "2026-10-28T03:00:00Z",
            "2026-10-28T04:00:00Z",
            &jiro_token,
        ),
    ] {
        let (status, slot) = send(
            &router,
            post_json(
                "/slots",
                coach,
                json!({ "sport": sport, "starts_at": starts_at, "ends_at": ends_at }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{slot}");
        let (status, booking) = send(
            &router,
            post_json(
                "/bookings",
                student,
                json!({ "slot_id": slot["slot_id"], "is_trial": false }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{booking}");
        booked.push(booking["booking_id"].clone());
    }
    // 3 件目（じろうの予約）をキャンセルする。キャンセルされた予約も競技で絞れる
    let (status, body) = send(
        &router,
        post_json(
            &format!("/bookings/{}/cancel", booked[2].as_str().unwrap()),
            &jiro_token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let ids_of = |body: &Value| -> Vec<Value> {
        body["bookings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["booking"]["booking_id"].clone())
            .collect()
    };
    let id = |i: usize| booked[i].clone();
    for (query, expected) in [
        // 枠のスポーツ（テニス）でなく、コーチの提供スポーツ（ゴルフ）で決める
        ("&sport=golf".to_string(), vec![id(0), id(2)]),
        ("&sport=tennis".to_string(), vec![id(0), id(2)]),
        ("&sport=soccer".to_string(), vec![id(1)]),
        ("&sport=baseball".to_string(), vec![]),
        // 付けなければ今までどおりすべての競技
        (String::new(), vec![id(0), id(1), id(2)]),
        // 期間・コーチ・生徒・状態と組み合わせる
        ("&sport=golf&to=2026-10-27".to_string(), vec![id(0)]),
        (
            format!("&sport=golf&coach_id={}", account_id_uuid::format(ito)),
            vec![],
        ),
        (
            format!("&sport=golf&coach_id={}", account_id_uuid::format(tanaka)),
            vec![id(0), id(2)],
        ),
        (
            format!("&sport=golf&student_id={}", account_id_uuid::format(taro)),
            vec![id(0)],
        ),
        ("&sport=golf&status=cancelled".to_string(), vec![id(2)]),
        ("&sport=soccer&status=cancelled".to_string(), vec![]),
    ] {
        let (status, body) = send(
            &router,
            get(&format!("/bookings?from=2026-10-26{query}"), &hq_token),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(ids_of(&body), expected, "{query}: {body}");
    }
    let (_, body) = send(&router, get("/bookings?sport=soccer", &hq_token)).await;
    assert_eq!(body["bookings"][0]["coach_display_name"], "ito", "{body}");

    for query in ["?sport=cricket", "?sport=Golf", "?sport="] {
        let (status, body) = send(&router, get(&format!("/bookings{query}"), &hq_token)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}: {body}");
        assert_eq!(body["code"], "validation_failed", "{query}");
    }
    let (status, _) = send(&router, get("/bookings?sport=golf", &tanaka_token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
