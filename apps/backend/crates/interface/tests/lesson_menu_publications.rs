//! `POST /lesson-menus/{lesson_menu_id}/unpublish`・`POST /lesson-menus/{lesson_menu_id}/republish`（admin-web の
//! `unpublishLessonMenu`・`republishLessonMenu`）の統合テスト。応答を admin-web の OpenAPI のスキーマで検査する。
//! 本部だけが呼べること（コーチ・生徒は 403）、ないメニューは 404、二度目は 409、本部が非公開にしたことと理由が
//! 本部の横断の一覧（`GET /lesson-menus`）に出ること、そのあいだコーチが `PUT /coaches/me/lesson-menus/{lesson_menu_id}`
//! で公開し直せないこと（409）と、生徒の一覧（`GET /coaches/{coach_id}/lesson-menus`）に出ないことを確かめる。

use std::time::{Duration, SystemTime};

use application::{
    catalog::{
        CreateLessonMenu, ListCoachLessonMenus, ListLessonMenus, ListMyLessonMenus,
        RepublishLessonMenu, UnpublishLessonMenu, UpdateLessonMenu,
    },
    identity::{AccountRepository, InvalidToken, TokenClaims, TokenVerifier},
    in_memory::{FixedClock, InMemoryAccountRepository, InMemoryLessonMenuRepository},
    profile::ProfileCoachDisplayNameLookup,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::{
        LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription, LessonMenuId,
        LessonMenuName, Sport,
    },
    identity::{Account, AccountId, Email, Role},
    profile::DisplayName,
};
use serde_json::{Value, json};
use tower::ServiceExt;

mod support;

/// 2026-10-26T00:00:00Z（現在時刻）
const NOW: Duration = Duration::from_secs(1_792_972_800);
const NOW_TEXT: &str = "2026-10-26T00:00:00Z";

/// `<役割>:<ID の数値>` の形のトークンを通す偽物（interface は infra のモックに依存できない）。
struct FakeTokens;

impl TokenVerifier for FakeTokens {
    fn verify(&self, token: &str) -> Result<TokenClaims, InvalidToken> {
        let (role, id) = token.split_once(':').ok_or(InvalidToken)?;
        let role = match role {
            "student" => Role::Student,
            "coach" => Role::Coach,
            "headquarters" => Role::Headquarters,
            _ => return Err(InvalidToken),
        };
        let id = id.parse().map_err(|_| InvalidToken)?;
        Ok(TokenClaims {
            account_id: AccountId::from_u128(id),
            role,
        })
    }
}

const HEADQUARTERS: &str = "headquarters:1";
const COACH: &str = "coach:2";
const COACH_ID: &str = "00000000-0000-0000-0000-000000000002";
const STUDENT: &str = "student:3";
const MENU_ID: &str = "00000000-0000-0000-0000-00000000000a";

fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + NOW
}

fn menu() -> LessonMenu {
    LessonMenu::restore(
        LessonMenuId::from_u128(10),
        AccountId::from_u128(2),
        LessonMenuContent {
            sport: Sport::Tennis,
            format: LessonFormat::Async,
            name: LessonMenuName::new("フォーム添削").unwrap(),
            description: LessonMenuDescription::new("初心者向け").unwrap(),
        },
        true,
        now(),
    )
}

/// 本部 1・コーチ 2・生徒 3 と、コーチ 2 の公開中のメニュー 1 件で組み立てた、catalog と本部の非公開のルーター。
async fn app() -> Router {
    let accounts = InMemoryAccountRepository::default();
    for (id, role, sports) in [
        (1, Role::Headquarters, vec![]),
        (2, Role::Coach, vec![Sport::Tennis]),
        (3, Role::Student, vec![Sport::Tennis]),
    ] {
        accounts
            .insert(
                Account::new(
                    AccountId::from_u128(id),
                    Email::new(format!("user{id}@example.com")).unwrap(),
                    DisplayName::new("たなか").unwrap(),
                    role,
                    sports,
                    now(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
    }
    let menus = InMemoryLessonMenuRepository::with_menus([menu()]);
    interface::catalog_router(
        CreateLessonMenu::new(menus.clone(), accounts.clone(), FixedClock(now())),
        UpdateLessonMenu::new(menus.clone(), accounts.clone()),
        ListMyLessonMenus::new(menus.clone()),
        ListCoachLessonMenus::new(menus.clone()),
        ListLessonMenus::new(menus.clone(), ProfileCoachDisplayNameLookup::new(accounts)),
        FakeTokens,
    )
    .merge(interface::lesson_menu_publication_router(
        UnpublishLessonMenu::new(menus.clone(), FixedClock(now())),
        RepublishLessonMenu::new(menus),
        FakeTokens,
    ))
}

/// リクエストを送り、応答を `contract`（契約の枝と、操作のパス）の契約で検査する。
async fn send(
    router: &Router,
    method: &str,
    uri: &str,
    contract: (&str, &str),
    token: Option<&str>,
    body: Option<Body>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = router
        .clone()
        .oneshot(request.body(body.unwrap_or_else(Body::empty)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let (app, path) = contract;
    support::Contract::load(app).assert_response(&method.to_lowercase(), path, status, &body);
    (status, body)
}

async fn unpublish(
    router: &Router,
    lesson_menu_id: &str,
    token: Option<&str>,
    body: Option<Body>,
) -> (StatusCode, Value) {
    send(
        router,
        "POST",
        &format!("/lesson-menus/{lesson_menu_id}/unpublish"),
        ("admin-web", "/lesson-menus/{lesson_menu_id}/unpublish"),
        token,
        body,
    )
    .await
}

async fn republish(
    router: &Router,
    lesson_menu_id: &str,
    token: Option<&str>,
) -> (StatusCode, Value) {
    send(
        router,
        "POST",
        &format!("/lesson-menus/{lesson_menu_id}/republish"),
        ("admin-web", "/lesson-menus/{lesson_menu_id}/republish"),
        token,
        None,
    )
    .await
}

fn json_body(value: Value) -> Option<Body> {
    Some(Body::from(value.to_string()))
}

async fn headquarters_list(router: &Router) -> Value {
    let (status, body) = send(
        router,
        "GET",
        "/lesson-menus",
        ("admin-web", "/lesson-menus"),
        Some(HEADQUARTERS),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn student_list(router: &Router) -> Value {
    let (status, body) = send(
        router,
        "GET",
        &format!("/coaches/{COACH_ID}/lesson-menus"),
        ("student-app", "/coaches/{coach_id}/lesson-menus"),
        Some(STUDENT),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn coach_update(router: &Router, is_published: bool) -> (StatusCode, Value) {
    send(
        router,
        "PUT",
        &format!("/coaches/me/lesson-menus/{MENU_ID}"),
        ("admin-web", "/coaches/me/lesson-menus/{lesson_menu_id}"),
        Some(COACH),
        json_body(json!({
            "sport": "tennis",
            "lesson_type": "async_lesson",
            "name": "フォーム添削（直した）",
            "description": "初心者向け",
            "is_published": is_published,
        })),
    )
    .await
}

#[tokio::test]
async fn headquarters_unpublishes_with_a_reason_and_republishes() {
    let router = app().await;

    let (status, body) = unpublish(
        &router,
        MENU_ID,
        Some(HEADQUARTERS),
        json_body(json!({ "reason": "料金の記載が誤っている" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["lesson_menu_id"], MENU_ID);
    assert_eq!(body["is_published"], false);
    assert_eq!(body["name"], "フォーム添削", "中身は変えない");
    assert_eq!(
        body["headquarters_unpublish"],
        json!({ "unpublished_at": NOW_TEXT, "reason": "料金の記載が誤っている" })
    );

    // 本部の横断の一覧に、非公開にしたことと理由が出る。生徒の一覧からは消える
    let list = headquarters_list(&router).await;
    assert_eq!(list["lesson_menus"][0]["is_published"], false);
    assert_eq!(
        list["lesson_menus"][0]["headquarters_unpublish"]["reason"],
        "料金の記載が誤っている"
    );
    assert_eq!(student_list(&router).await["lesson_menus"], json!([]));

    let (status, body) = republish(&router, MENU_ID, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["is_published"], true);
    assert!(
        body.get("headquarters_unpublish").is_none(),
        "公開に戻すと項目ごと消える: {body}"
    );
    assert!(
        headquarters_list(&router).await["lesson_menus"][0]
            .get("headquarters_unpublish")
            .is_none()
    );
    assert_eq!(
        student_list(&router).await["lesson_menus"][0]["lesson_menu_id"],
        MENU_ID
    );
}

#[tokio::test]
async fn the_body_and_the_reason_may_be_omitted() {
    for body in [
        None,
        json_body(json!({})),
        json_body(json!({ "reason": null })),
    ] {
        let router = app().await;
        let (status, body) = unpublish(&router, MENU_ID, Some(HEADQUARTERS), body).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["headquarters_unpublish"]["reason"], "");
    }
}

#[tokio::test]
async fn a_too_long_reason_or_a_broken_body_is_422_and_changes_nothing() {
    let router = app().await;
    for body in [
        json_body(json!({ "reason": "あ".repeat(201) })),
        Some(Body::from("{")),
        json_body(json!({ "reason": 1 })),
    ] {
        let (status, body) = unpublish(&router, MENU_ID, Some(HEADQUARTERS), body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(body["code"], "validation_failed");
    }
    let (status, _) = unpublish(
        &router,
        MENU_ID,
        Some(HEADQUARTERS),
        json_body(json!({ "reason": "あ".repeat(200) })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "200 文字までは通る");
}

#[tokio::test]
async fn unpublishing_twice_or_republishing_a_menu_headquarters_did_not_unpublish_is_409() {
    let router = app().await;

    let (status, body) = republish(&router, MENU_ID, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "lesson_menu_not_unpublished_by_headquarters");

    unpublish(
        &router,
        MENU_ID,
        Some(HEADQUARTERS),
        json_body(json!({ "reason": "最初" })),
    )
    .await;
    let (status, body) = unpublish(
        &router,
        MENU_ID,
        Some(HEADQUARTERS),
        json_body(json!({ "reason": "二度目" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["code"],
        "lesson_menu_already_unpublished_by_headquarters"
    );
    assert_eq!(
        headquarters_list(&router).await["lesson_menus"][0]["headquarters_unpublish"]["reason"],
        "最初",
        "最初の理由を残す"
    );
}

#[tokio::test]
async fn the_coach_cannot_republish_but_can_fix_the_content() {
    let router = app().await;
    unpublish(
        &router,
        MENU_ID,
        Some(HEADQUARTERS),
        json_body(json!({ "reason": "誤った記載" })),
    )
    .await;

    let (status, body) = coach_update(&router, true).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "lesson_menu_unpublished_by_headquarters");

    let (status, body) = coach_update(&router, false).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["name"], "フォーム添削（直した）");
    assert_eq!(body["is_published"], false);
    assert_eq!(body["headquarters_unpublish"]["reason"], "誤った記載");

    // 本部が公開に戻したあとは、コーチがまた公開・非公開を決められる
    republish(&router, MENU_ID, Some(HEADQUARTERS)).await;
    let (status, body) = coach_update(&router, false).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = coach_update(&router, true).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["is_published"], true);
}

#[tokio::test]
async fn a_missing_or_malformed_menu_is_404() {
    let router = app().await;
    for lesson_menu_id in ["00000000-0000-0000-0000-000000000099", "not-a-uuid"] {
        let (status, body) = unpublish(&router, lesson_menu_id, Some(HEADQUARTERS), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{lesson_menu_id}: {body}");
        assert_eq!(body["code"], "not_found");
        let (status, body) = republish(&router, lesson_menu_id, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{lesson_menu_id}: {body}");
        assert_eq!(body["code"], "not_found");
    }
}

#[tokio::test]
async fn only_headquarters_may_call_and_a_token_is_required() {
    let router = app().await;
    for token in [COACH, STUDENT] {
        let (status, body) = unpublish(&router, MENU_ID, Some(token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
        let (status, body) = republish(&router, MENU_ID, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
    }
    let (status, body) = unpublish(&router, MENU_ID, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    let (status, body) = republish(&router, MENU_ID, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(
        student_list(&router).await["lesson_menus"][0]["is_published"],
        true,
        "何も変えない"
    );
}
