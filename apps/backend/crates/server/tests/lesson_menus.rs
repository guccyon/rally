//! コーチのレッスンメニュー（catalog）を、保存先を Postgres にした組み立て（`server::router_with_postgres`）で通す E2E。
//! コーチが admin-web の `POST /coaches/me/lesson-menus` で作り、`PUT /coaches/me/lesson-menus/{lesson_menu_id}` で直して
//! 非公開にし、`GET /coaches/me/lesson-menus` で読めること、生徒が student-app の `GET /coaches/{coach_id}/lesson-menus` で
//! 公開中のメニューだけを（スポーツで絞って）読めること、生徒と本部はメニューを作れず直せず、別のコーチはメニューを直せない
//! ことを確かめる。契約の形の細かい検査は interface の `tests/lesson_menus.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test lesson_menus -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    clock::SystemClock,
    identity::{RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer},
    in_memory::{ManualClock, RecordingAnalyticsSink},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{Role, Sport};
use infra::{MockTokenService, PgAccountRepository};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

async fn send(
    router: &Router,
    method: &str,
    path: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json");
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

/// 役割 `role` のアカウントを登録してトークンを返す。契約にコーチ・本部の登録の操作がないので、ユースケースを直に呼ぶ。
async fn register(pool: &PgPool, email: &str, role: Role, sports: Vec<Sport>) -> String {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        SystemClock,
    )
    .execute(RegisterAccountCommand {
        email: email.to_string(),
        display_name: "テスト".to_string(),
        role,
        sports,
    })
    .await
    .unwrap();
    MockTokenService
        .issue(TokenClaims {
            account_id: account.id(),
            role,
        })
        .into_string()
}

/// 2026-11-01T00:00:00Z から `minutes` 分後。
fn at(minutes: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200 + minutes * 60)
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn coaches_publish_lesson_menus_and_students_read_the_published_ones(pool: PgPool) {
    let coach = register(
        &pool,
        "coach@example.com",
        Role::Coach,
        vec![Sport::Tennis, Sport::Golf],
    )
    .await;
    let other_coach = register(
        &pool,
        "coach2@example.com",
        Role::Coach,
        vec![Sport::Tennis],
    )
    .await;
    let headquarters = register(&pool, "hq@example.com", Role::Headquarters, vec![]).await;
    let student = register(
        &pool,
        "taro@example.com",
        Role::Student,
        vec![Sport::Tennis],
    )
    .await;
    // Postgres の時刻はマイクロ秒までなので、作った時刻は秒で揃え、並べ方が決まるようにメニューごとに進める
    let clock = ManualClock::new(at(0));
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    // コーチが同期・非同期のメニューを作る
    let (status, live) = send(
        &router,
        "POST",
        "/coaches/me/lesson-menus",
        &coach,
        Some(json!({
            "sport": "tennis",
            "lesson_type": "live_lesson",
            "name": "フォーム改善",
            "description": "通話でフォームを直す",
            "duration_minutes": 45,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{live}");
    clock.set(at(1));
    let coach_id = live["coach_id"].as_str().unwrap().to_string();
    let live_id = live["lesson_menu_id"].as_str().unwrap().to_string();
    let (status, review) = send(
        &router,
        "POST",
        "/coaches/me/lesson-menus",
        &coach,
        Some(json!({
            "sport": "tennis",
            "lesson_type": "async_lesson",
            "name": "動画添削",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{review}");
    clock.set(at(2));
    let (status, golf) = send(
        &router,
        "POST",
        "/coaches/me/lesson-menus",
        &coach,
        Some(json!({
            "sport": "golf",
            "lesson_type": "async_lesson",
            "name": "スイング添削",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{golf}");

    // 提供スポーツでないメニューは 422
    let (status, body) = send(
        &router,
        "POST",
        "/coaches/me/lesson-menus",
        &coach,
        Some(json!({"sport": "soccer", "lesson_type": "async_lesson", "name": "添削"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    // 生徒と本部は作れず、直せず、コーチの一覧も読めない
    for token in [&student, &headquarters] {
        let (status, body) = send(
            &router,
            "POST",
            "/coaches/me/lesson-menus",
            token,
            Some(json!({"sport": "tennis", "lesson_type": "async_lesson", "name": "添削"})),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        let (status, _) = send(
            &router,
            "PUT",
            &format!("/coaches/me/lesson-menus/{live_id}"),
            token,
            Some(json!({"name": "乗っ取り", "description": "", "duration_minutes": 30, "is_published": false})),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = send(&router, "GET", "/coaches/me/lesson-menus", token, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    // 別のコーチは直せない（あることも知らせない）
    let (status, _) = send(
        &router,
        "PUT",
        &format!("/coaches/me/lesson-menus/{live_id}"),
        &other_coach,
        Some(json!({"name": "乗っ取り", "description": "", "duration_minutes": 30, "is_published": false})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // 生徒には公開中のテニスのメニューが出る（ゴルフは student-app の範囲外）
    let path = format!("/coaches/{coach_id}/lesson-menus");
    let (status, list) = send(&router, "GET", &path, &student, None).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list["lesson_menus"], json!([live, review]));

    clock.set(at(3));
    // コーチが同期レッスンを非公開にすると、生徒の一覧から消え、コーチの一覧には残る
    let (status, hidden) = send(
        &router,
        "PUT",
        &format!("/coaches/me/lesson-menus/{live_id}"),
        &coach,
        Some(json!({
            "name": "フォーム改善",
            "description": "通話でフォームを直す",
            "duration_minutes": 60,
            "is_published": false,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{hidden}");
    assert_eq!(hidden["duration_minutes"], 60);
    assert_eq!(hidden["is_published"], false);

    let (_, list) = send(
        &router,
        "GET",
        &format!("{path}?sport=tennis"),
        &student,
        None,
    )
    .await;
    assert_eq!(list["lesson_menus"], json!([review]));
    let (status, mine) = send(&router, "GET", "/coaches/me/lesson-menus", &coach, None).await;
    assert_eq!(status, StatusCode::OK, "{mine}");
    assert_eq!(mine["lesson_menus"], json!([hidden, review, golf]));
    let (_, others) = send(
        &router,
        "GET",
        "/coaches/me/lesson-menus",
        &other_coach,
        None,
    )
    .await;
    assert_eq!(others["lesson_menus"], json!([]));

    // コーチは student-app の一覧を呼べない
    let (status, _) = send(&router, "GET", &path, &coach, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
