//! 本部によるレッスンメニューの非公開・公開に戻す操作（admin-web の `POST /lesson-menus/{lesson_menu_id}/unpublish`・
//! `/republish`）を、保存先を Postgres にした組み立て（`server::router_with_postgres`）で通す。本部が理由を付けて非公開にした
//! メニューが生徒の一覧（`GET /coaches/{coach_id}/lesson-menus`）から消え、本部の横断の一覧（`GET /lesson-menus`）とコーチの
//! 自分の一覧（`GET /coaches/me/lesson-menus`）に非公開にした時刻と理由が出ること、そのあいだコーチは公開し直せない（409）が
//! 中身は直せること、本部が公開に戻すと生徒の一覧に戻ること、非公開にした・公開に戻した記録（誰が・いつ・理由）が
//! `GET /lesson-menus/{lesson_menu_id}/publication-changes` で新しい順に読めること（本部だけ）を確かめる。契約の形の細かい検査は interface の
//! `tests/lesson_menu_publications.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test lesson_menu_publications -- --include-ignored` で走らせる。

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
use domain::{catalog::Sport, identity::Role};
use infra::{MockTokenService, PgAccountRepository};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

/// 2026-10-30T00:00:00Z から `secs` 秒後（Postgres の時刻はマイクロ秒までなので秒単位にする）。
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

/// 役割 `role` のアカウントを登録して（ID, トークン）を返す。契約にコーチ・本部の登録の操作がないので、ユースケースを直に呼ぶ。
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
    (account_id_uuid::format(account.id()), token)
}

fn menu_request(name: &str, is_published: bool) -> Value {
    json!({
        "sport": "tennis",
        "lesson_type": "async_lesson",
        "name": name,
        "description": "",
        "is_published": is_published,
    })
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_unpublishes_and_republishes_a_coach_menu(pool: PgPool) {
    let (coach_id, coach) =
        register(&pool, "coach@example.com", Role::Coach, vec![Sport::Tennis]).await;
    let (headquarters_id, headquarters) =
        register(&pool, "hq@example.com", Role::Headquarters, vec![]).await;
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

    let (status, menu) = send(
        &router,
        "POST",
        "/coaches/me/lesson-menus",
        &coach,
        Some(menu_request("フォーム添削", true)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{menu}");
    let menu_id = menu["lesson_menu_id"].as_str().unwrap().to_string();
    let student_menus = format!("/coaches/{coach_id}/lesson-menus");

    // コーチ・生徒は非公開にできない
    for token in [&coach, &student] {
        let (status, body) = send(
            &router,
            "POST",
            &format!("/lesson-menus/{menu_id}/unpublish"),
            token,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }

    clock.set(at(60));
    let (status, body) = send(
        &router,
        "POST",
        &format!("/lesson-menus/{menu_id}/unpublish"),
        &headquarters,
        Some(json!({ "reason": "誤解を招く表現がある" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let unpublish =
        json!({ "unpublished_at": "2026-10-30T00:01:00Z", "reason": "誤解を招く表現がある" });
    assert_eq!(body["headquarters_unpublish"], unpublish);
    assert_eq!(body["is_published"], false);

    // 生徒には見えず、本部とコーチには非公開にした時刻と理由が見える
    let (_, list) = send(&router, "GET", &student_menus, &student, None).await;
    assert_eq!(list["lesson_menus"], json!([]));
    let (_, list) = send(
        &router,
        "GET",
        "/lesson-menus?published=false",
        &headquarters,
        None,
    )
    .await;
    assert_eq!(list["lesson_menus"][0]["headquarters_unpublish"], unpublish);
    let (_, list) = send(&router, "GET", "/coaches/me/lesson-menus", &coach, None).await;
    assert_eq!(list["lesson_menus"][0]["headquarters_unpublish"], unpublish);

    // コーチは公開し直せないが、非公開のまま中身は直せる
    let path = format!("/coaches/me/lesson-menus/{menu_id}");
    let (status, body) = send(
        &router,
        "PUT",
        &path,
        &coach,
        Some(menu_request("フォーム添削（直した）", true)),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "lesson_menu_unpublished_by_headquarters");
    let (status, body) = send(
        &router,
        "PUT",
        &path,
        &coach,
        Some(menu_request("フォーム添削（直した）", false)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["headquarters_unpublish"], unpublish);

    // 本部が公開に戻すと、生徒の一覧に直した中身で戻る
    clock.set(at(120));
    let (status, body) = send(
        &router,
        "POST",
        &format!("/lesson-menus/{menu_id}/republish"),
        &headquarters,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["is_published"], true);
    assert!(body.get("headquarters_unpublish").is_none(), "{body}");
    let (_, list) = send(&router, "GET", &student_menus, &student, None).await;
    assert_eq!(list["lesson_menus"][0]["lesson_menu_id"], menu_id);
    assert_eq!(list["lesson_menus"][0]["name"], "フォーム添削（直した）");

    // 二度目の公開に戻す操作は 409
    let (status, body) = send(
        &router,
        "POST",
        &format!("/lesson-menus/{menu_id}/republish"),
        &headquarters,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "lesson_menu_not_unpublished_by_headquarters");

    // 非公開にした・公開に戻した記録が新しい順に残る（断った操作とコーチ自身の変更は載らない）
    let changes_path = format!("/lesson-menus/{menu_id}/publication-changes");
    let (status, body) = send(&router, "GET", &changes_path, &headquarters, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "lesson_menu_id": menu_id,
            "changes": [
                {
                    "action": "republished",
                    "reason": null,
                    "changed_by": headquarters_id,
                    "changed_at": "2026-10-30T00:02:00Z",
                },
                {
                    "action": "unpublished",
                    "reason": "誤解を招く表現がある",
                    "changed_by": headquarters_id,
                    "changed_at": "2026-10-30T00:01:00Z",
                },
            ],
        })
    );
    for token in [&coach, &student] {
        let (status, body) = send(&router, "GET", &changes_path, token, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
    let (status, body) = send(
        &router,
        "GET",
        "/lesson-menus/00000000-0000-0000-0000-000000000099/publication-changes",
        &headquarters,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}
