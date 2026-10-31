//! コーチのレッスンメニュー（catalog）を、保存先を Postgres にした組み立て（`server::router_with_postgres`）で通す。
//! コーチが admin-web の操作でメニューを作り・直し・非公開にし、生徒が student-app の操作で公開中のものだけを
//! コーチごと・スポーツごとに読めること、生徒・本部はメニューを作れず直せないこと（403）、他のコーチのメニューは
//! 直せないこと（404）を確かめる。契約の形の細かい検査は interface の `tests/lesson_menus.rs`。
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
use domain::{catalog::Sport, identity::Role};
use infra::{MockTokenService, PgAccountRepository};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

/// 2026-10-30T00:00:00Z から `secs` 秒後（Postgres の時刻はマイクロ秒までなので、読み戻しと比べられる秒単位にする）。
fn at(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_318_400 + secs)
}

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

/// 役割 `role` のアカウントを登録して、アカウント ID とトークンを返す。契約にコーチ・本部の登録の操作がないので、
/// ユースケースを直に呼ぶ。
async fn register(pool: &PgPool, email: &str, role: Role, sports: Vec<Sport>) -> (String, String) {
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
    let token = MockTokenService
        .issue(TokenClaims {
            account_id: account.id(),
            role,
        })
        .into_string();
    (
        application::identity::account_id_uuid::format(account.id()),
        token,
    )
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn coaches_manage_their_menus_and_students_read_the_published_ones(pool: PgPool) {
    let (coach_id, coach) = register(
        &pool,
        "coach@example.com",
        Role::Coach,
        vec![Sport::Tennis, Sport::Swimming],
    )
    .await;
    let (_, other_coach) =
        register(&pool, "other@example.com", Role::Coach, vec![Sport::Tennis]).await;
    let (_, headquarters) = register(&pool, "hq@example.com", Role::Headquarters, vec![]).await;
    let (_, student) = register(
        &pool,
        "taro@example.com",
        Role::Student,
        vec![Sport::Tennis],
    )
    .await;
    let clock = ManualClock::new(at(0));
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    let (status, live) = send(
        &router,
        "POST",
        "/coaches/me/lesson-menus",
        &coach,
        Some(json!({
            "sport": "tennis",
            "lesson_type": "live_lesson",
            "name": "フォア改善",
            "description": "ラリーが続かない人向け",
            "duration_minutes": 45,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{live}");
    clock.set(at(60));
    let (status, swim) = send(
        &router,
        "POST",
        "/coaches/me/lesson-menus",
        &coach,
        Some(
            json!({ "sport": "swimming", "lesson_type": "async_lesson", "name": "クロールの添削" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{swim}");

    // 生徒はコーチの公開中のメニューを読め、スポーツで絞れる
    let menus_of = format!("/coaches/{coach_id}/lesson-menus");
    let (status, listed) = send(&router, "GET", &menus_of, &student, None).await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_eq!(listed["lesson_menus"], json!([live, swim]));
    let (_, tennis) = send(
        &router,
        "GET",
        &format!("{menus_of}?sport=tennis"),
        &student,
        None,
    )
    .await;
    assert_eq!(tennis["lesson_menus"], json!([live]));

    // 生徒と本部は作れず直せない。他のコーチは直せない（あることも知らせない）
    let menu_path = format!(
        "/coaches/me/lesson-menus/{}",
        live["lesson_menu_id"].as_str().unwrap()
    );
    let hide = json!({
        "sport": "tennis",
        "lesson_type": "live_lesson",
        "name": "フォア改善",
        "description": "ラリーが続かない人向け",
        "duration_minutes": 45,
        "is_published": false,
    });
    for token in [&student, &headquarters] {
        let (status, body) = send(&router, "PUT", &menu_path, token, Some(hide.clone())).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        let (status, body) = send(
            &router,
            "POST",
            "/coaches/me/lesson-menus",
            token,
            Some(hide.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
    let (status, body) = send(&router, "PUT", &menu_path, &other_coach, Some(hide.clone())).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (_, listed) = send(&router, "GET", &menus_of, &student, None).await;
    assert_eq!(
        listed["lesson_menus"],
        json!([live, swim]),
        "何も変わらない"
    );

    // コーチが非公開にすると、生徒には見えず、コーチの一覧には残る
    let (status, hidden) = send(&router, "PUT", &menu_path, &coach, Some(hide)).await;
    assert_eq!(status, StatusCode::OK, "{hidden}");
    assert_eq!(hidden["is_published"], false);
    let (_, listed) = send(&router, "GET", &menus_of, &student, None).await;
    assert_eq!(listed["lesson_menus"], json!([swim]));
    let (status, mine) = send(&router, "GET", "/coaches/me/lesson-menus", &coach, None).await;
    assert_eq!(status, StatusCode::OK, "{mine}");
    assert_eq!(mine["lesson_menus"], json!([hidden, swim]));

    // 提供スポーツでないメニューは作れない（DB にも入らない）
    let (status, body) = send(
        &router,
        "POST",
        "/coaches/me/lesson-menus",
        &coach,
        Some(json!({ "sport": "soccer", "lesson_type": "async_lesson", "name": "リフティング" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
}
