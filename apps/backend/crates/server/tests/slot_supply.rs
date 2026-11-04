//! `GET /slot-supply` を組み立て済みのルーター（`server::router()`）で通す。トークンは infra のモック
//! （`MockTokenService`）で発行する。契約の形・数え方の検査は interface の `tests/slot_supply.rs`、
//! 合成シードでの判定は seed の `tests/slot_supply.rs`。日付（`from`・`to`）で期間を指定して Postgres の枠と予約を数える
//! ことと、`coach_id` でそのコーチの枠と予約だけを数えることと、`sport` でその競技を提供するコーチの枠と予約だけを数える
//! ことと、`lesson_menu_id` でそのメニューに結びついた枠と予約だけを数えることは、保存先を Postgres にした組み立てで
//! 確かめる。

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

async fn get_slot_supply(role: Role) -> (StatusCode, Value) {
    let token = MockTokenService
        .issue(TokenClaims {
            account_id: AccountId::from_u128(42),
            role,
        })
        .into_string();
    let request = Request::get("/slot-supply")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let response = server::router().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn headquarters_reads_the_supply_of_the_six_sports() {
    let (status, body) = get_slot_supply(Role::Headquarters).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let sports: Vec<&str> = body["sports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["sport"].as_str().unwrap())
        .collect();
    assert_eq!(
        sports,
        [
            "tennis",
            "golf",
            "baseball",
            "soccer",
            "basketball",
            "swimming"
        ]
    );
}

#[tokio::test]
async fn coaches_and_students_may_not_read_the_supply() {
    for role in [Role::Coach, Role::Student] {
        let (status, body) = get_slot_supply(role).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{role:?}: {body}");
    }
}

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

/// アカウントを登録してトークンを返す。契約にコーチ・本部の登録の操作がないので、ユースケースを直に呼ぶ。
async fn register(pool: &PgPool, email: &str, role: Role) -> String {
    register_account(pool, email, role).await.1
}

/// アカウントを登録して、その ID とトークンを返す。
async fn register_account(pool: &PgPool, email: &str, role: Role) -> (AccountId, String) {
    let sports = if role == Role::Headquarters {
        vec![]
    } else {
        vec![Sport::Tennis]
    };
    register_offering(pool, email, role, sports).await
}

/// 提供スポーツ（生徒ならスポーツ）を指定してアカウントを登録して、その ID とトークンを返す。
async fn register_offering(
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

/// 枠を公開して、その ID を返す。
async fn publish_slot(router: &Router, coach: &str, starts_at: &str, ends_at: &str) -> Value {
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
    slot["slot_id"].clone()
}

/// 枠を予約する。
async fn book(router: &Router, student: &str, slot_id: &Value) {
    let (status, booking) = send(
        router,
        post_json(
            "/bookings",
            student,
            json!({ "slot_id": slot_id, "is_trial": false }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");
}

/// 保存先を Postgres にした組み立て（`server::router_with_postgres`）で、来週の日付（日本時間）を指定して需給を読む。
/// Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
/// `cargo test -p server --test slot_supply -- --include-ignored` で走らせる。
#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_reads_the_supply_of_next_week_from_postgres(pool: PgPool) {
    let coach = register(&pool, "tanaka@example.com", Role::Coach).await;
    let student = register(&pool, "taro@example.com", Role::Student).await;
    let hq = register(&pool, "hq@example.com", Role::Headquarters).await;
    let router = server::router_with_postgres(
        pool,
        FixedClock(SystemTime::UNIX_EPOCH + NOW),
        RecordingAnalyticsSink::default(),
    );

    // 来週（日本時間の 11 月 2 日から 8 日）の始まりちょうど（2 日 0 時）と終わりの日の夜（8 日 22 時）に 1 つずつ、
    // その直後（9 日 0 時）に 1 つ
    let mut slot_ids = Vec::new();
    for (starts_at, ends_at) in [
        ("2026-11-01T15:00:00Z", "2026-11-01T16:00:00Z"),
        ("2026-11-08T13:00:00Z", "2026-11-08T14:00:00Z"),
        ("2026-11-08T15:00:00Z", "2026-11-08T16:00:00Z"),
    ] {
        let (status, slot) = send(
            &router,
            post_json(
                "/slots",
                &coach,
                json!({ "sport": "tennis", "starts_at": starts_at, "ends_at": ends_at }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{slot}");
        slot_ids.push(slot["slot_id"].clone());
    }
    // 期間の外の枠への予約は数えない
    for (slot_id, is_trial) in [(&slot_ids[0], true), (&slot_ids[2], false)] {
        let (status, booking) = send(
            &router,
            post_json(
                "/bookings",
                &student,
                json!({ "slot_id": slot_id, "is_trial": is_trial }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{booking}");
    }

    let (status, body) = send(
        &router,
        get("/slot-supply?from=2026-11-02&to=2026-11-08", &hq),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["from"], "2026-11-02");
    assert_eq!(body["to"], "2026-11-08");
    assert_eq!(body["window_start"], "2026-11-01T15:00:00Z");
    assert_eq!(body["window_end"], "2026-11-08T15:00:00Z");
    // 公開した 2 枠に 1 人（50%）
    assert_eq!(
        body["sports"][0],
        json!({ "sport": "tennis", "status": "sufficient", "published_slots": 2, "booking_students": 1 })
    );

    // 両方省けば今までどおり、今までの 30 日（来週の枠は入らない）
    let (status, body) = send(&router, get("/slot-supply", &hq)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["window_end"], "2026-10-26T00:00:00Z");
    assert!(body.get("from").is_none(), "{body}");
    assert_eq!(body["sports"][0]["published_slots"], 0, "{body}");

    // 期間の違反は 422
    let (status, body) = send(
        &router,
        get("/slot-supply?from=2026-11-02&to=2026-12-03", &hq),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");
}

/// 保存先を Postgres にした組み立てで、コーチ（`coach_id`）で絞って来週の需給を読む。
/// Postgres が要るので既定では走らない（`#[ignore]`）。
#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_reads_the_supply_of_a_coach_from_postgres(pool: PgPool) {
    let (tanaka_id, tanaka) = register_account(&pool, "tanaka@example.com", Role::Coach).await;
    let suzuki = register(&pool, "suzuki@example.com", Role::Coach).await;
    let (taro_id, taro) = register_account(&pool, "taro@example.com", Role::Student).await;
    let hanako = register(&pool, "hanako@example.com", Role::Student).await;
    let hq = register(&pool, "hq@example.com", Role::Headquarters).await;
    let router = server::router_with_postgres(
        pool,
        FixedClock(SystemTime::UNIX_EPOCH + NOW),
        RecordingAnalyticsSink::default(),
    );

    // 来週（日本時間の 11 月 2 日から 8 日）に、たなかの枠 2 つ（1 つに 1 人）と、すずきの枠 1 つ（別の生徒が 1 人）
    let tanaka_first = publish_slot(
        &router,
        &tanaka,
        "2026-11-02T01:00:00Z",
        "2026-11-02T02:00:00Z",
    )
    .await;
    publish_slot(
        &router,
        &tanaka,
        "2026-11-03T01:00:00Z",
        "2026-11-03T02:00:00Z",
    )
    .await;
    let suzuki_slot = publish_slot(
        &router,
        &suzuki,
        "2026-11-02T01:00:00Z",
        "2026-11-02T02:00:00Z",
    )
    .await;
    book(&router, &taro, &tanaka_first).await;
    book(&router, &hanako, &suzuki_slot).await;

    let week = "from=2026-11-02&to=2026-11-08";
    let (status, body) = send(
        &router,
        get(
            &format!(
                "/slot-supply?{week}&coach_id={}",
                account_id_uuid::format(tanaka_id)
            ),
            &hq,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["from"], "2026-11-02");
    assert_eq!(body["window_start"], "2026-11-01T15:00:00Z");
    // たなかの 2 枠に 1 人（50%）。すずきの枠とその予約は数えない
    assert_eq!(
        body["sports"][0],
        json!({ "sport": "tennis", "status": "sufficient", "published_slots": 2, "booking_students": 1 })
    );

    // 付けなければ今までどおりすべてのコーチ（3 枠に 2 人）
    let (status, body) = send(&router, get(&format!("/slot-supply?{week}"), &hq)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["sports"][0],
        json!({ "sport": "tennis", "status": "insufficient", "published_slots": 3, "booking_students": 2 })
    );

    // コーチでない ID・ない ID は 404 にせず、どのスポーツも 0
    for coach_id in [
        account_id_uuid::format(taro_id),
        account_id_uuid::format(AccountId::from_u128(1000)),
    ] {
        let (status, body) = send(
            &router,
            get(&format!("/slot-supply?{week}&coach_id={coach_id}"), &hq),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{coach_id}: {body}");
        assert_eq!(body["sports"][0]["published_slots"], 0, "{body}");
        assert_eq!(body["sports"][0]["booking_students"], 0, "{body}");
    }

    // UUID でなければ 422
    let (status, body) = send(&router, get("/slot-supply?coach_id=tanaka", &hq)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");
    assert_eq!(body["message"], "coach_id が UUID でない");
}

/// 保存先を Postgres にした組み立てで、競技（`sport`）で絞って来週の需給を読む。コーチの提供スポーツは Postgres の
/// アカウントから読む。Postgres が要るので既定では走らない（`#[ignore]`）。
#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_reads_the_supply_of_a_sport_from_postgres(pool: PgPool) {
    // たなかはテニスとサッカー、すずきはテニスだけを提供する
    let (tanaka_id, tanaka) = register_offering(
        &pool,
        "tanaka@example.com",
        Role::Coach,
        vec![Sport::Tennis, Sport::Soccer],
    )
    .await;
    let suzuki = register(&pool, "suzuki@example.com", Role::Coach).await;
    let taro = register(&pool, "taro@example.com", Role::Student).await;
    let hanako = register(&pool, "hanako@example.com", Role::Student).await;
    let hq = register(&pool, "hq@example.com", Role::Headquarters).await;
    let router = server::router_with_postgres(
        pool,
        FixedClock(SystemTime::UNIX_EPOCH + NOW),
        RecordingAnalyticsSink::default(),
    );

    // 来週に、たなかのテニスの枠 2 つ（1 つに 1 人）と、すずきのテニスの枠 1 つ（別の生徒が 1 人）
    let tanaka_first = publish_slot(
        &router,
        &tanaka,
        "2026-11-02T01:00:00Z",
        "2026-11-02T02:00:00Z",
    )
    .await;
    publish_slot(
        &router,
        &tanaka,
        "2026-11-03T01:00:00Z",
        "2026-11-03T02:00:00Z",
    )
    .await;
    let suzuki_slot = publish_slot(
        &router,
        &suzuki,
        "2026-11-02T01:00:00Z",
        "2026-11-02T02:00:00Z",
    )
    .await;
    book(&router, &taro, &tanaka_first).await;
    book(&router, &hanako, &suzuki_slot).await;

    let week = "from=2026-11-02&to=2026-11-08";
    // サッカーを提供するのはたなかだけ。たなかのテニスの枠を数える（枠のスポーツでは絞らない）
    let (status, body) = send(
        &router,
        get(&format!("/slot-supply?{week}&sport=soccer"), &hq),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["from"], "2026-11-02");
    assert_eq!(body["window_start"], "2026-11-01T15:00:00Z");
    assert_eq!(
        body["sports"][0],
        json!({ "sport": "tennis", "status": "sufficient", "published_slots": 2, "booking_students": 1 })
    );
    assert_eq!(body["sports"][3]["published_slots"], 0, "{body}");

    // テニスはたなかとすずき（3 枠に 2 人）。付けないときと同じ
    let (status, body) = send(
        &router,
        get(&format!("/slot-supply?{week}&sport=tennis"), &hq),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["sports"][0],
        json!({ "sport": "tennis", "status": "insufficient", "published_slots": 3, "booking_students": 2 })
    );

    // コーチと組み合わせる。たなかでサッカーならたなかの枠、提供するコーチのいない競技なら 0
    let (status, body) = send(
        &router,
        get(
            &format!(
                "/slot-supply?{week}&sport=soccer&coach_id={}",
                account_id_uuid::format(tanaka_id)
            ),
            &hq,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["sports"][0]["published_slots"], 2, "{body}");
    let (status, body) = send(
        &router,
        get(&format!("/slot-supply?{week}&sport=golf"), &hq),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["sports"][0]["published_slots"], 0, "{body}");
    assert_eq!(body["sports"][0]["booking_students"], 0, "{body}");

    // `Sport` の値でなければ 422
    let (status, body) = send(&router, get("/slot-supply?sport=cricket", &hq)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");
    assert_eq!(body["message"], "未知のスポーツ: cricket");
}

/// 保存先を Postgres にした組み立てで、レッスンメニュー（`lesson_menu_id`）で絞って来週の需給を読む。枠とメニューの
/// 結びつきは Postgres の枠から読む。Postgres が要るので既定では走らない（`#[ignore]`）。
#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_reads_the_supply_of_a_lesson_menu_from_postgres(pool: PgPool) {
    let (tanaka_id, tanaka) = register_account(&pool, "tanaka@example.com", Role::Coach).await;
    let (suzuki_id, _) = register_account(&pool, "suzuki@example.com", Role::Coach).await;
    let taro = register(&pool, "taro@example.com", Role::Student).await;
    let hanako = register(&pool, "hanako@example.com", Role::Student).await;
    let hq = register(&pool, "hq@example.com", Role::Headquarters).await;
    let router = server::router_with_postgres(
        pool,
        FixedClock(SystemTime::UNIX_EPOCH + NOW),
        RecordingAnalyticsSink::default(),
    );

    // たなかが同期レッスンのメニューを作り、来週の 2 枠に結びつける（1 つに 1 人）。メニューのない枠にも 1 人
    let (status, menu) = send(
        &router,
        post_json(
            "/coaches/me/lesson-menus",
            &tanaka,
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
    let mut menu_slots = Vec::new();
    for (starts_at, ends_at) in [
        ("2026-11-02T01:00:00Z", "2026-11-02T02:00:00Z"),
        ("2026-11-03T01:00:00Z", "2026-11-03T02:00:00Z"),
    ] {
        let (status, slot) = send(
            &router,
            post_json(
                "/slots",
                &tanaka,
                json!({
                    "sport": "tennis",
                    "starts_at": starts_at,
                    "ends_at": ends_at,
                    "lesson_menu_id": menu_id,
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{slot}");
        menu_slots.push(slot["slot_id"].clone());
    }
    let plain_slot = publish_slot(
        &router,
        &tanaka,
        "2026-11-04T01:00:00Z",
        "2026-11-04T02:00:00Z",
    )
    .await;
    book(&router, &taro, &menu_slots[0]).await;
    book(&router, &hanako, &plain_slot).await;

    let week = "from=2026-11-02&to=2026-11-08";
    let supply_of = |query: String| {
        let (router, hq) = (&router, &hq);
        async move {
            let (status, body) = send(router, get(&format!("/slot-supply?{query}"), hq)).await;
            assert_eq!(status, StatusCode::OK, "{query}: {body}");
            body
        }
    };
    // メニューの 2 枠に 1 人（50%）。メニューのない枠とその予約は数えない
    let body = supply_of(format!("{week}&lesson_menu_id={menu_id}")).await;
    assert_eq!(body["from"], "2026-11-02");
    assert_eq!(body["window_start"], "2026-11-01T15:00:00Z");
    assert_eq!(
        body["sports"][0],
        json!({ "sport": "tennis", "status": "sufficient", "published_slots": 2, "booking_students": 1 })
    );
    // コーチ・競技と組み合わせる
    let body = supply_of(format!(
        "{week}&lesson_menu_id={menu_id}&coach_id={}&sport=tennis",
        account_id_uuid::format(tanaka_id)
    ))
    .await;
    assert_eq!(body["sports"][0]["published_slots"], 2, "{body}");
    let body = supply_of(format!(
        "{week}&lesson_menu_id={menu_id}&coach_id={}",
        account_id_uuid::format(suzuki_id)
    ))
    .await;
    assert_eq!(body["sports"][0]["published_slots"], 0, "{body}");
    // 期間と組み合わせる（11 月 3 日からは 11 月 2 日の枠とその予約を含まない）
    let body = supply_of(format!(
        "from=2026-11-03&to=2026-11-08&lesson_menu_id={menu_id}"
    ))
    .await;
    assert_eq!(
        body["sports"][0],
        json!({ "sport": "tennis", "status": "sufficient", "published_slots": 1, "booking_students": 0 })
    );
    // ないメニューの ID は 404 にせず、どのスポーツも 0
    let body = supply_of(format!(
        "{week}&lesson_menu_id=00000000-0000-0000-0000-0000000003e7"
    ))
    .await;
    assert!(
        body["sports"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["published_slots"] == 0 && s["booking_students"] == 0),
        "{body}"
    );
    // 付けなければ今までどおりメニューを問わない（3 枠に 2 人）
    let body = supply_of(week.to_string()).await;
    assert_eq!(
        body["sports"][0],
        json!({ "sport": "tennis", "status": "insufficient", "published_slots": 3, "booking_students": 2 })
    );

    // UUID でなければ 422
    let (status, body) = send(&router, get("/slot-supply?lesson_menu_id=menu", &hq)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");
    assert_eq!(body["message"], "lesson_menu_id が UUID でない");
}
