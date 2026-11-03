//! 本部がキャンセルされた予約の一覧をキャンセルした日付で見る（admin-web の `listCancelledBookings`）ことを、保存先を
//! Postgres にした組み立て（`server::router_with_postgres`）で通す。ルートが組み立てに入っていること（同じ階層の
//! `GET /bookings/{booking_id}` と並んでも当たること）、生徒が student-app の `cancelBooking` でキャンセルした予約を
//! Postgres からキャンセルした日本時間の日付で絞ってキャンセルの新しい順に読み、生徒・コーチの表示名とキャンセルした人を
//! 添えること、キャンセルしていない予約は出ないこと、コーチ・生徒・レッスンメニュー（予約した時点の写し。期間と組み合わせて）で
//! 絞れること、`sport` で Postgres に保存したコーチの提供スポーツがその競技のキャンセルされた予約だけに絞れること（ほかの条件と
//! 組み合わせて）、本部以外は 403 になることを確かめる。
//! 契約の形・期間の規則の細かい検査は interface の `tests/headquarters_cancelled_bookings.rs`、期間にキャンセルした予約の
//! 読み取りは infra の `tests/pg_booking_repository.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test headquarters_cancelled_bookings -- --include-ignored` で走らせる。

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

fn at(offset: Duration) -> SystemTime {
    SystemTime::UNIX_EPOCH + NOW + offset
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
async fn headquarters_reads_cancelled_bookings_by_cancellation_date_from_postgres(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (ito, ito_token) = register(&pool, "ito@example.com", Role::Coach).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;
    let (_, hq_token) = register(&pool, "hq@example.com", Role::Headquarters).await;
    let clock = ManualClock::new(at(Duration::ZERO));
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    // 11 月の枠を 3 つ予約する（枠の開始はキャンセルした日付と関係しない）
    let mut booking_ids = Vec::new();
    for (coach, starts_at, ends_at) in [
        (
            &tanaka_token,
            "2026-11-10T01:00:00Z",
            "2026-11-10T02:00:00Z",
        ),
        (&ito_token, "2026-11-11T01:00:00Z", "2026-11-11T02:00:00Z"),
        (
            &tanaka_token,
            "2026-11-12T01:00:00Z",
            "2026-11-12T02:00:00Z",
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
        booking_ids.push(booking["booking_id"].as_str().unwrap().to_string());
    }

    // 1 つ目を日本時間の 10 月 26 日 10 時に、2 つ目を 10 月 27 日 9 時にキャンセルする。3 つ目はキャンセルしない
    for (booking_id, cancelled_at) in [
        (&booking_ids[0], at(HOUR)),
        (&booking_ids[1], at(24 * HOUR)),
    ] {
        clock.set(cancelled_at);
        let (status, body) = send(
            &router,
            post_json(
                &format!("/bookings/{booking_id}/cancel"),
                &taro_token,
                json!({}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    // 今日（日本時間の 10 月 27 日）までの 7 日
    let (status, body) = send(&router, get("/bookings/cancelled", &hq_token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (&body["from"], &body["to"]),
        (&json!("2026-10-21"), &json!("2026-10-27"))
    );
    assert_eq!(
        body["bookings"],
        json!([
            {
                "booking_id": booking_ids[1],
                "student_id": account_id_uuid::format(taro),
                "student_display_name": "taro",
                "coach_id": account_id_uuid::format(ito),
                "coach_display_name": "ito",
                "starts_at": "2026-11-11T01:00:00Z",
                "cancelled_at": "2026-10-27T00:00:00Z",
                "cancelled_by": "student",
            },
            {
                "booking_id": booking_ids[0],
                "student_id": account_id_uuid::format(taro),
                "student_display_name": "taro",
                "coach_id": account_id_uuid::format(tanaka),
                "coach_display_name": "tanaka",
                "starts_at": "2026-11-10T01:00:00Z",
                "cancelled_at": "2026-10-26T01:00:00Z",
                "cancelled_by": "student",
            },
        ]),
        "キャンセルの新しい順。キャンセルしていない予約は出ない"
    );

    // 日付で絞る
    let (status, body) = send(
        &router,
        get(
            "/bookings/cancelled?from=2026-10-26&to=2026-10-26",
            &hq_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["bookings"].as_array().unwrap().len(), 1, "{body}");
    assert_eq!(body["bookings"][0]["booking_id"], booking_ids[0].as_str());

    // コーチで絞る（期間は省いたときと同じ今日までの 7 日。キャンセルしていないたなかの 3 つ目は出ない）
    for (coach_id, expected) in [
        (account_id_uuid::format(tanaka), vec![&booking_ids[0]]),
        (account_id_uuid::format(ito), vec![&booking_ids[1]]),
        // コーチでない ID（生徒）は 404 にせず空の配列
        (account_id_uuid::format(taro), vec![]),
    ] {
        let (status, body) = send(
            &router,
            get(
                &format!("/bookings/cancelled?coach_id={coach_id}"),
                &hq_token,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            (&body["from"], &body["to"]),
            (&json!("2026-10-21"), &json!("2026-10-27"))
        );
        let ids: Vec<&str> = body["bookings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["booking_id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, expected, "{coach_id}: {body}");
    }
    let (status, body) = send(
        &router,
        get("/bookings/cancelled?coach_id=tanaka", &hq_token),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["message"], "coach_id が UUID でない");

    // 同じ階層の予約 1 件の詳細も読める
    let (status, body) = send(
        &router,
        get(&format!("/bookings/{}", booking_ids[0]), &hq_token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["booking"]["status"], "cancelled");

    // 本部以外は 403
    for token in [&tanaka_token, &taro_token] {
        let (status, body) = send(&router, get("/bookings/cancelled", token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_narrows_cancelled_bookings_to_a_student_from_postgres(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (ito, ito_token) = register(&pool, "ito@example.com", Role::Coach).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;
    let (hanako, hanako_token) = register(&pool, "hanako@example.com", Role::Student).await;
    let (_, hq_token) = register(&pool, "hq@example.com", Role::Headquarters).await;
    let clock = ManualClock::new(at(Duration::ZERO));
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    // 4 つの枠を予約し、日本時間の 10 月 26 日〜27 日に 3 つをキャンセルする（たろうのたなかの 2 つ目はキャンセルしない）
    let mut booking_ids = Vec::new();
    for (coach, student, starts_at, cancelled_at) in [
        (
            &tanaka_token,
            &taro_token,
            "2026-11-10T01:00:00Z",
            Some(at(HOUR)),
        ),
        (
            &ito_token,
            &taro_token,
            "2026-11-11T01:00:00Z",
            Some(at(24 * HOUR)),
        ),
        (
            &tanaka_token,
            &hanako_token,
            "2026-11-12T01:00:00Z",
            Some(at(2 * HOUR)),
        ),
        (&tanaka_token, &taro_token, "2026-11-13T01:00:00Z", None),
    ] {
        clock.set(at(Duration::ZERO));
        let ends_at = starts_at.replace("T01:", "T02:");
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
        let booking_id = booking["booking_id"].as_str().unwrap().to_string();
        if let Some(cancelled_at) = cancelled_at {
            clock.set(cancelled_at);
            let (status, body) = send(
                &router,
                post_json(
                    &format!("/bookings/{booking_id}/cancel"),
                    student,
                    json!({}),
                ),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
        }
        booking_ids.push(booking_id);
    }
    // 今日は日本時間の 10 月 27 日
    clock.set(at(24 * HOUR));

    let taro_id = account_id_uuid::format(taro);
    let hanako_id = account_id_uuid::format(hanako);
    for (query, expected) in [
        // 期間は省いたときと同じ今日までの 7 日。キャンセルの新しい順で、キャンセルしていない予約は出ない
        (
            format!("student_id={taro_id}"),
            vec![&booking_ids[1], &booking_ids[0]],
        ),
        (format!("student_id={hanako_id}"), vec![&booking_ids[2]]),
        // 期間・コーチと組み合わせる
        (
            format!("student_id={taro_id}&from=2026-10-26&to=2026-10-26"),
            vec![&booking_ids[0]],
        ),
        (
            format!(
                "student_id={taro_id}&coach_id={}",
                account_id_uuid::format(tanaka)
            ),
            vec![&booking_ids[0]],
        ),
        (
            format!(
                "student_id={hanako_id}&coach_id={}",
                account_id_uuid::format(ito)
            ),
            vec![],
        ),
        // 生徒でない ID（コーチ）は 404 にせず空の配列
        (
            format!("student_id={}", account_id_uuid::format(tanaka)),
            vec![],
        ),
    ] {
        let (status, body) = send(
            &router,
            get(&format!("/bookings/cancelled?{query}"), &hq_token),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        let ids: Vec<&str> = body["bookings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["booking_id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, expected, "{query}: {body}");
    }
    let (status, body) = send(
        &router,
        get("/bookings/cancelled?student_id=taro", &hq_token),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["message"], "student_id が UUID でない");
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_narrows_cancelled_bookings_to_a_lesson_menu_from_postgres(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (ito, ito_token) = register(&pool, "ito@example.com", Role::Coach).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;
    let (_, hanako_token) = register(&pool, "hanako@example.com", Role::Student).await;
    let (_, hq_token) = register(&pool, "hq@example.com", Role::Headquarters).await;
    let clock = ManualClock::new(at(Duration::ZERO));
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    // たなかが同期レッスンのメニューを作る
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

    // 5 つの枠を予約し、日本時間の 10 月 26 日〜27 日に 4 つをキャンセルする。メニューを結びつけた枠の予約は、予約した時点の
    // メニューの写しを残す（たろうのメニューの 2 つ目はキャンセルしない）
    let mut booking_ids = Vec::new();
    for (coach, with_menu, student, starts_at, cancelled_at) in [
        (
            &tanaka_token,
            true,
            &taro_token,
            "2026-11-10T01:00:00Z",
            Some(at(HOUR)),
        ),
        (
            &tanaka_token,
            true,
            &hanako_token,
            "2026-11-11T01:00:00Z",
            Some(at(24 * HOUR)),
        ),
        // メニューのない枠
        (
            &tanaka_token,
            false,
            &taro_token,
            "2026-11-12T01:00:00Z",
            Some(at(2 * HOUR)),
        ),
        (
            &ito_token,
            false,
            &taro_token,
            "2026-11-13T01:00:00Z",
            Some(at(3 * HOUR)),
        ),
        (
            &tanaka_token,
            true,
            &taro_token,
            "2026-11-14T01:00:00Z",
            None,
        ),
    ] {
        clock.set(at(Duration::ZERO));
        let ends_at = starts_at.replace("T01:", "T02:");
        let mut slot = json!({ "sport": "tennis", "starts_at": starts_at, "ends_at": ends_at });
        if with_menu {
            slot["lesson_menu_id"] = json!(menu_id);
        }
        let (status, slot) = send(&router, post_json("/slots", coach, slot)).await;
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
        let booking_id = booking["booking_id"].as_str().unwrap().to_string();
        if let Some(cancelled_at) = cancelled_at {
            clock.set(cancelled_at);
            let (status, body) = send(
                &router,
                post_json(
                    &format!("/bookings/{booking_id}/cancel"),
                    student,
                    json!({}),
                ),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
        }
        booking_ids.push(booking_id);
    }
    // 今日は日本時間の 10 月 27 日
    clock.set(at(24 * HOUR));

    let taro_id = account_id_uuid::format(taro);
    for (query, expected) in [
        // 期間は省いたときと同じ今日までの 7 日。キャンセルの新しい順で、メニューのない予約・キャンセルしていない予約は出ない
        (
            format!("lesson_menu_id={menu_id}"),
            vec![&booking_ids[1], &booking_ids[0]],
        ),
        // 期間・コーチ・生徒と組み合わせる
        (
            format!("lesson_menu_id={menu_id}&from=2026-10-27&to=2026-10-27"),
            vec![&booking_ids[1]],
        ),
        (
            format!(
                "lesson_menu_id={menu_id}&coach_id={}&student_id={taro_id}",
                account_id_uuid::format(tanaka)
            ),
            vec![&booking_ids[0]],
        ),
        (
            format!(
                "lesson_menu_id={menu_id}&coach_id={}",
                account_id_uuid::format(ito)
            ),
            vec![],
        ),
        // ないメニューの ID は 404 にせず空の配列
        (
            "lesson_menu_id=00000000-0000-0000-0000-0000000003e7".to_string(),
            vec![],
        ),
        // 付けなければ今までどおりメニューを問わない
        (
            String::new(),
            vec![
                &booking_ids[1],
                &booking_ids[3],
                &booking_ids[2],
                &booking_ids[0],
            ],
        ),
    ] {
        let (status, body) = send(
            &router,
            get(&format!("/bookings/cancelled?{query}"), &hq_token),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        let ids: Vec<&str> = body["bookings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["booking_id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, expected, "{query}: {body}");
    }
    let (status, body) = send(
        &router,
        get("/bookings/cancelled?lesson_menu_id=menu", &hq_token),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["message"], "lesson_menu_id が UUID でない");
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_narrows_cancelled_bookings_by_the_sport_of_the_coach_in_postgres(
    pool: PgPool,
) {
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
    let clock = ManualClock::new(at(Duration::ZERO));
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    // 11 月にたなかの枠（テニス）・いとうの枠（サッカー）・たなかの枠（テニス）・たなかの枠（テニス）を予約する
    let mut booked = Vec::new();
    for (coach, sport, starts_at, ends_at, student) in [
        (
            &tanaka_token,
            "tennis",
            "2026-11-10T01:00:00Z",
            "2026-11-10T02:00:00Z",
            &taro_token,
        ),
        (
            &ito_token,
            "soccer",
            "2026-11-11T01:00:00Z",
            "2026-11-11T02:00:00Z",
            &taro_token,
        ),
        (
            &tanaka_token,
            "tennis",
            "2026-11-12T01:00:00Z",
            "2026-11-12T02:00:00Z",
            &jiro_token,
        ),
        (
            &tanaka_token,
            "tennis",
            "2026-11-13T01:00:00Z",
            "2026-11-13T02:00:00Z",
            &taro_token,
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
        booked.push((booking["booking_id"].clone(), student));
    }

    // 1 件目を日本時間の 10 月 26 日 10 時、2 件目を 10 月 27 日 9 時、3 件目を 10 月 27 日 10 時にキャンセルする。
    // 4 件目（たなかの枠）はキャンセルしない
    for (i, cancelled_at) in [(0, at(HOUR)), (1, at(24 * HOUR)), (2, at(25 * HOUR))] {
        clock.set(cancelled_at);
        let (booking_id, student) = &booked[i];
        let (status, body) = send(
            &router,
            post_json(
                &format!("/bookings/{}/cancel", booking_id.as_str().unwrap()),
                student,
                json!({}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    let ids_of = |body: &Value| -> Vec<Value> {
        body["bookings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["booking_id"].clone())
            .collect()
    };
    let id = |i: usize| booked[i].0.clone();
    for (query, expected) in [
        // 枠のスポーツ（テニス）でなく、コーチの提供スポーツ（ゴルフ）で決める。キャンセルの新しい順は変わらない
        ("&sport=golf".to_string(), vec![id(2), id(0)]),
        ("&sport=tennis".to_string(), vec![id(2), id(0)]),
        ("&sport=soccer".to_string(), vec![id(1)]),
        ("&sport=baseball".to_string(), vec![]),
        // 付けなければ今までどおりすべての競技
        (String::new(), vec![id(2), id(1), id(0)]),
        // 期間・コーチ・生徒と組み合わせる
        ("&sport=golf&to=2026-10-26".to_string(), vec![id(0)]),
        (
            format!("&sport=golf&coach_id={}", account_id_uuid::format(ito)),
            vec![],
        ),
        (
            format!("&sport=golf&coach_id={}", account_id_uuid::format(tanaka)),
            vec![id(2), id(0)],
        ),
        (
            format!("&sport=golf&student_id={}", account_id_uuid::format(taro)),
            vec![id(0)],
        ),
        (
            format!("&sport=soccer&student_id={}", account_id_uuid::format(taro)),
            vec![id(1)],
        ),
    ] {
        let (status, body) = send(
            &router,
            get(
                &format!("/bookings/cancelled?from=2026-10-26{query}"),
                &hq_token,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(ids_of(&body), expected, "{query}: {body}");
    }
    let (_, body) = send(&router, get("/bookings/cancelled?sport=soccer", &hq_token)).await;
    assert_eq!(body["bookings"][0]["coach_display_name"], "ito", "{body}");

    for query in ["?sport=cricket", "?sport=Golf", "?sport="] {
        let (status, body) = send(
            &router,
            get(&format!("/bookings/cancelled{query}"), &hq_token),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}: {body}");
        assert_eq!(body["code"], "validation_failed", "{query}");
    }
    let (status, _) = send(
        &router,
        get("/bookings/cancelled?sport=golf", &tanaka_token),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
