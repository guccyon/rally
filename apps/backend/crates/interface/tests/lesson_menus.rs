//! コーチのレッスンメニュー（catalog）の統合テスト。admin-web の `GET`・`POST /coaches/me/lesson-menus` と
//! `PUT /coaches/me/lesson-menus/{lesson_menu_id}`（コーチだけ）、student-app の `GET /coaches/{coach_id}/lesson-menus`
//! （生徒だけ・公開中だけ）を、レスポンスの形をそれぞれの OpenAPI のスキーマで検査しながら通す。
//! 生徒・本部がメニューを作る・直すことができないこと（403）、他のコーチのメニューを直せないこと（404）を確かめる。
//! admin-web の `GET /lesson-menus`（本部だけ・すべてのコーチの横断）の絞り込み（スポーツ・公開中か・コーチ）と並べ方、各行の直近の予約の数、
//! コーチ・生徒が読めないこと（403）も確かめる。

use std::time::{Duration, SystemTime};

use application::{
    catalog::{
        CreateLessonMenu, ListCoachLessonMenus, ListLessonMenus, ListMyLessonMenus,
        UpdateLessonMenu,
    },
    identity::{AccountRepository, InvalidToken, TokenClaims, TokenVerifier},
    in_memory::{
        FixedClock, InMemoryAccountRepository, InMemoryBookingRepository,
        InMemoryLessonMenuRepository, InMemorySlotRepository,
    },
    profile::ProfileCoachDisplayNameLookup,
    scheduling::{BookingRepository, SchedulingLessonMenuRecentBookingCountLookup},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    catalog::{
        LessonDuration, LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription,
        LessonMenuId, LessonMenuName,
    },
    identity::{Account, AccountId, Email, Role},
    lesson::LessonType,
    profile::DisplayName,
    scheduling::{Booking, Slot, SlotId, SlotSeats, SlotStatus},
};
use serde_json::{Value, json};
use tower::ServiceExt;

mod support;

/// 2026-10-26T00:00:00Z（現在時刻）
const NOW: Duration = Duration::from_secs(1_792_972_800);
/// 1 日（秒）
const DAY: u64 = 24 * 3600;

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

/// テニス・ゴルフのコーチ「たなか」（ID 42 = 0x2a）
const COACH: &str = "coach:42";
const COACH_ID: &str = "00000000-0000-0000-0000-00000000002a";
/// テニスのコーチ「いとう」（ID 43 = 0x2b）
const OTHER_COACH: &str = "coach:43";
const OTHER_COACH_ID: &str = "00000000-0000-0000-0000-00000000002b";
const STUDENT: &str = "student:3";
const HEADQUARTERS: &str = "headquarters:1";
/// アカウントのないコーチのトークン
const UNKNOWN_COACH: &str = "coach:99";

fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + NOW
}

async fn accounts() -> InMemoryAccountRepository {
    let accounts = InMemoryAccountRepository::default();
    for (id, name, role, sports) in [
        (1, "テスト", Role::Headquarters, vec![]),
        (3, "テスト", Role::Student, vec![Sport::Tennis]),
        (42, "たなか", Role::Coach, vec![Sport::Tennis, Sport::Golf]),
        (43, "いとう", Role::Coach, vec![Sport::Tennis]),
    ] {
        accounts
            .insert(
                Account::new(
                    AccountId::from_u128(id),
                    Email::new(format!("user{id}@example.com")).unwrap(),
                    DisplayName::new(name).unwrap(),
                    role,
                    sports,
                    now(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
    }
    accounts
}

async fn app() -> Router {
    app_with(InMemoryLessonMenuRepository::default()).await
}

async fn app_with(menus: InMemoryLessonMenuRepository) -> Router {
    app_with_bookings(
        menus,
        InMemoryBookingRepository::new(InMemorySlotRepository::default()),
    )
    .await
}

async fn app_with_bookings(
    menus: InMemoryLessonMenuRepository,
    bookings: InMemoryBookingRepository,
) -> Router {
    let accounts = accounts().await;
    interface::catalog_router(
        CreateLessonMenu::new(menus.clone(), accounts.clone(), FixedClock(now())),
        UpdateLessonMenu::new(menus.clone(), accounts.clone()),
        ListMyLessonMenus::new(menus.clone()),
        ListCoachLessonMenus::new(menus.clone()),
        ListLessonMenus::new(
            menus,
            ProfileCoachDisplayNameLookup::new(accounts),
            SchedulingLessonMenuRecentBookingCountLookup::new(bookings),
            FixedClock(now()),
        ),
        FakeTokens,
    )
}

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, body)
}

fn live_menu(sport: &str, name: &str) -> Value {
    json!({
        "sport": sport,
        "lesson_type": "live_lesson",
        "name": name,
        "description": "初心者向け",
        "duration_minutes": 60,
        "is_published": true,
    })
}

const ADMIN_MENUS: &str = "/coaches/me/lesson-menus";
const ADMIN_MENU: &str = "/coaches/me/lesson-menus/{lesson_menu_id}";
const STUDENT_MENUS: &str = "/coaches/{coach_id}/lesson-menus";
const HEADQUARTERS_MENUS: &str = "/lesson-menus";

#[tokio::test]
async fn the_coach_creates_lists_and_unpublishes_its_menus() {
    let admin = support::Contract::load("admin-web");
    let student_app = support::Contract::load("student-app");
    let app = app().await;

    // 説明と公開中かを省くと空文字列と公開中
    let (status, created) = send(
        &app,
        "POST",
        ADMIN_MENUS,
        COACH,
        Some(json!({ "sport": "tennis", "lesson_type": "async_lesson", "name": "フォームの添削" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    admin.assert_response("post", ADMIN_MENUS, status, &created);
    assert_eq!(created["coach_id"], COACH_ID);
    assert_eq!(created["description"], "");
    assert_eq!(created["duration_minutes"], Value::Null);
    assert_eq!(created["is_published"], true);

    let (status, live) = send(
        &app,
        "POST",
        ADMIN_MENUS,
        COACH,
        Some(live_menu("tennis", "フォア改善")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{live}");
    assert_eq!(live["lesson_type"], "live_lesson");
    assert_eq!(live["duration_minutes"], 60);

    // ゴルフのメニューは admin-web では作れるが、student-app には載せない
    let (status, golf) = send(
        &app,
        "POST",
        ADMIN_MENUS,
        COACH,
        Some(live_menu("golf", "スイング")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{golf}");

    // 非公開にする
    let id = live["lesson_menu_id"].as_str().unwrap();
    let mut hidden = live_menu("tennis", "フォア改善（準備中）");
    hidden["is_published"] = json!(false);
    let (status, updated) = send(
        &app,
        "PUT",
        &format!("{ADMIN_MENUS}/{id}"),
        COACH,
        Some(hidden),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    admin.assert_response("put", ADMIN_MENU, status, &updated);
    assert_eq!(updated["lesson_menu_id"], id);
    assert_eq!(updated["is_published"], false);
    assert_eq!(updated["name"], "フォア改善（準備中）");
    assert_eq!(updated["created_at"], live["created_at"]);

    // コーチの一覧には非公開も含めてすべて
    let (status, mine) = send(&app, "GET", ADMIN_MENUS, COACH, None).await;
    assert_eq!(status, StatusCode::OK, "{mine}");
    admin.assert_response("get", ADMIN_MENUS, status, &mine);
    assert_eq!(mine["coach_id"], COACH_ID);
    assert_eq!(mine["lesson_menus"].as_array().unwrap().len(), 3);

    // 生徒には公開中で、ゴルフでないものだけ
    let (status, listed) = send(
        &app,
        "GET",
        &format!("/coaches/{COACH_ID}/lesson-menus"),
        STUDENT,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    student_app.assert_response("get", STUDENT_MENUS, status, &listed);
    assert_eq!(listed["lesson_menus"], json!([created]));

    // スポーツで絞る
    let (status, filtered) = send(
        &app,
        "GET",
        &format!("/coaches/{COACH_ID}/lesson-menus?sport=swimming"),
        STUDENT,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{filtered}");
    assert_eq!(filtered["lesson_menus"], json!([]));
    let (status, body) = send(
        &app,
        "GET",
        &format!("/coaches/{COACH_ID}/lesson-menus?sport=golf"),
        STUDENT,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    student_app.assert_response("get", STUDENT_MENUS, status, &body);
}

#[tokio::test]
async fn students_and_headquarters_cannot_write_menus() {
    let admin = support::Contract::load("admin-web");
    let app = app().await;
    let (_, menu) = send(
        &app,
        "POST",
        ADMIN_MENUS,
        COACH,
        Some(live_menu("tennis", "フォア")),
    )
    .await;
    let path = format!("{ADMIN_MENUS}/{}", menu["lesson_menu_id"].as_str().unwrap());

    for token in [STUDENT, HEADQUARTERS] {
        let (status, body) = send(
            &app,
            "POST",
            ADMIN_MENUS,
            token,
            Some(live_menu("tennis", "乗っ取り")),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        admin.assert_response("post", ADMIN_MENUS, status, &body);
        let (status, body) = send(
            &app,
            "PUT",
            &path,
            token,
            Some(live_menu("tennis", "乗っ取り")),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        admin.assert_response("put", ADMIN_MENU, status, &body);
        let (status, body) = send(&app, "GET", ADMIN_MENUS, token, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
    }
    // 生徒向けの一覧はコーチ・本部には出さない
    for token in [COACH, HEADQUARTERS] {
        let (status, body) = send(
            &app,
            "GET",
            &format!("/coaches/{COACH_ID}/lesson-menus"),
            token,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
    }

    let (_, mine) = send(&app, "GET", ADMIN_MENUS, COACH, None).await;
    assert_eq!(mine["lesson_menus"], json!([menu]), "何も変わらない");
}

#[tokio::test]
async fn another_coach_cannot_see_that_the_menu_exists() {
    let admin = support::Contract::load("admin-web");
    let app = app().await;
    let (_, menu) = send(
        &app,
        "POST",
        ADMIN_MENUS,
        COACH,
        Some(live_menu("tennis", "フォア")),
    )
    .await;
    let path = format!("{ADMIN_MENUS}/{}", menu["lesson_menu_id"].as_str().unwrap());

    let (status, body) = send(
        &app,
        "PUT",
        &path,
        OTHER_COACH,
        Some(live_menu("tennis", "乗っ取り")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    admin.assert_response("put", ADMIN_MENU, status, &body);
    for path in [
        format!("{ADMIN_MENUS}/00000000-0000-0000-0000-000000000063"),
        format!("{ADMIN_MENUS}/not-a-uuid"),
    ] {
        let (status, body) = send(&app, "PUT", &path, COACH, Some(live_menu("tennis", "x"))).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
    }
    let (_, others) = send(&app, "GET", ADMIN_MENUS, OTHER_COACH, None).await;
    assert_eq!(others["lesson_menus"], json!([]));
}

#[tokio::test]
async fn invalid_menus_are_422_and_nothing_is_saved() {
    let admin = support::Contract::load("admin-web");
    let app = app().await;
    let cases = [
        // コーチの提供スポーツでない
        live_menu("soccer", "リフティング"),
        json!({ "sport": "cricket", "lesson_type": "async_lesson", "name": "x" }),
        json!({ "sport": "tennis", "lesson_type": "group_lesson", "name": "x" }),
        // 同期なのに時間がない・非同期なのに時間がある・刻みに合わない
        json!({ "sport": "tennis", "lesson_type": "live_lesson", "name": "x" }),
        json!({ "sport": "tennis", "lesson_type": "async_lesson", "name": "x", "duration_minutes": 30 }),
        json!({ "sport": "tennis", "lesson_type": "live_lesson", "name": "x", "duration_minutes": 20 }),
        json!({ "sport": "tennis", "lesson_type": "live_lesson", "name": "x", "duration_minutes": 135 }),
        // 名前は 1〜40 文字、説明は 400 文字まで
        json!({ "sport": "tennis", "lesson_type": "async_lesson", "name": " " }),
        json!({ "sport": "tennis", "lesson_type": "async_lesson", "name": "あ".repeat(41) }),
        json!({ "sport": "tennis", "lesson_type": "async_lesson", "name": "x", "description": "あ".repeat(401) }),
        json!({ "sport": "tennis", "lesson_type": "async_lesson" }),
    ];
    for body in cases {
        let (status, response) = send(&app, "POST", ADMIN_MENUS, COACH, Some(body.clone())).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{body}: {response}"
        );
        assert_eq!(response["code"], "validation_failed", "{body}");
        admin.assert_response("post", ADMIN_MENUS, status, &response);
    }
    // 上限ちょうどは通る
    let (status, body) = send(
        &app,
        "POST",
        ADMIN_MENUS,
        COACH,
        Some(json!({
            "sport": "tennis",
            "lesson_type": "live_lesson",
            "name": "あ".repeat(40),
            "description": "あ".repeat(400),
            "duration_minutes": 120,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    // 変更でも同じ規則で、拒否すれば何も変えない
    let path = format!("{ADMIN_MENUS}/{}", body["lesson_menu_id"].as_str().unwrap());
    let (status, response) = send(
        &app,
        "PUT",
        &path,
        COACH,
        Some(live_menu("soccer", "リフティング")),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{response}");
    admin.assert_response("put", ADMIN_MENU, status, &response);
    let (_, mine) = send(&app, "GET", ADMIN_MENUS, COACH, None).await;
    assert_eq!(mine["lesson_menus"], json!([body]));
}

#[tokio::test]
async fn a_coach_without_an_account_is_401_and_unknown_coaches_have_no_menus() {
    let app = app().await;
    let (status, body) = send(
        &app,
        "POST",
        ADMIN_MENUS,
        UNKNOWN_COACH,
        Some(live_menu("tennis", "x")),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "unauthorized");

    let (status, body) = send(
        &app,
        "GET",
        "/coaches/00000000-0000-0000-0000-000000000063/lesson-menus",
        STUDENT,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["lesson_menus"], json!([]));
    let (status, _) = send(
        &app,
        "GET",
        "/coaches/not-a-uuid/lesson-menus",
        STUDENT,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(&app, "GET", ADMIN_MENUS, "nope", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn headquarters_lists_menus_of_every_coach_by_sport_then_coach_name() {
    let admin = support::Contract::load("admin-web");
    let app = app().await;
    let (_, forehand) = send(
        &app,
        "POST",
        ADMIN_MENUS,
        COACH,
        Some(live_menu("tennis", "フォア")),
    )
    .await;
    let (_, swing) = send(
        &app,
        "POST",
        ADMIN_MENUS,
        COACH,
        Some(live_menu("golf", "スイング")),
    )
    .await;
    let (_, review) = send(
        &app,
        "POST",
        ADMIN_MENUS,
        OTHER_COACH,
        Some(json!({
            "sport": "tennis",
            "lesson_type": "async_lesson",
            "name": "フォームの添削",
            "is_published": false,
        })),
    )
    .await;

    let (status, listed) = send(&app, "GET", HEADQUARTERS_MENUS, HEADQUARTERS, None).await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    admin.assert_response("get", HEADQUARTERS_MENUS, status, &listed);
    // テニス（いとう → たなか）→ ゴルフ。ゴルフも載せる
    assert_eq!(
        listed["lesson_menus"],
        json!([
            {
                "lesson_menu_id": review["lesson_menu_id"],
                "name": "フォームの添削",
                "sport": "tennis",
                "lesson_type": "async_lesson",
                "duration_minutes": null,
                "is_published": false,
                "coach_id": OTHER_COACH_ID,
                "coach_display_name": "いとう",
                "created_at": review["created_at"],
                "recent_booking_count": 0,
            },
            {
                "lesson_menu_id": forehand["lesson_menu_id"],
                "name": "フォア",
                "sport": "tennis",
                "lesson_type": "live_lesson",
                "duration_minutes": 60,
                "is_published": true,
                "coach_id": COACH_ID,
                "coach_display_name": "たなか",
                "created_at": forehand["created_at"],
                "recent_booking_count": 0,
            },
            {
                "lesson_menu_id": swing["lesson_menu_id"],
                "name": "スイング",
                "sport": "golf",
                "lesson_type": "live_lesson",
                "duration_minutes": 60,
                "is_published": true,
                "coach_id": COACH_ID,
                "coach_display_name": "たなか",
                "created_at": swing["created_at"],
                "recent_booking_count": 0,
            },
        ])
    );

    let names = |body: &Value| -> Vec<String> {
        body["lesson_menus"]
            .as_array()
            .unwrap()
            .iter()
            .map(|menu| menu["name"].as_str().unwrap().to_string())
            .collect()
    };
    for (query, expected) in [
        ("?sport=golf", vec!["スイング"]),
        ("?published=false", vec!["フォームの添削"]),
        ("?published=true", vec!["フォア", "スイング"]),
        ("?sport=tennis&published=true", vec!["フォア"]),
        ("?sport=swimming", vec![]),
        // コーチで絞ると公開中も非公開もまとめて。sport・published と組み合わせられる
        (&format!("?coach_id={COACH_ID}"), vec!["フォア", "スイング"]),
        (
            &format!("?coach_id={OTHER_COACH_ID}"),
            vec!["フォームの添削"],
        ),
        (
            &format!("?coach_id={COACH_ID}&sport=golf"),
            vec!["スイング"],
        ),
        (
            &format!("?coach_id={OTHER_COACH_ID}&published=true"),
            vec![],
        ),
        // いないコーチは 404 にせず空の一覧
        ("?coach_id=00000000-0000-0000-0000-000000000063", vec![]),
        // 名前に部分一致（「フォ」）。並べ方は変わらず、ほかの絞り込みと組み合わせられる
        ("?q=%E3%83%95%E3%82%A9", vec!["フォームの添削", "フォア"]),
        ("?q=%E3%83%95%E3%82%A9&published=true", vec!["フォア"]),
        (
            &format!("?q=%E3%83%95%E3%82%A9&coach_id={COACH_ID}&sport=golf"),
            vec![],
        ),
        // 前後の空白（全角を含む）は除いて比べる
        ("?q=%E3%80%80%E3%82%A4%E3%83%B3%E3%82%B0+", vec!["スイング"]),
        ("?q=%E3%81%84%E3%81%AA%E3%81%84", vec![]),
        // 空・空白だけなら絞り込まない
        ("?q=", vec!["フォームの添削", "フォア", "スイング"]),
        (
            "?q=+%E3%80%80+",
            vec!["フォームの添削", "フォア", "スイング"],
        ),
        // 前後の空白を除いて 40 文字までは受け付ける
        (&format!("?q=+{}+", "a".repeat(40)), vec![]),
    ] {
        let (status, body) = send(
            &app,
            "GET",
            &format!("{HEADQUARTERS_MENUS}{query}"),
            HEADQUARTERS,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        admin.assert_response("get", HEADQUARTERS_MENUS, status, &body);
        assert_eq!(names(&body), expected, "{query}");
    }

    let too_long = format!("?q={}", "a".repeat(41));
    for query in [
        "?sport=cricket",
        "?published=yes",
        "?coach_id=not-a-uuid",
        too_long.as_str(),
    ] {
        let (status, body) = send(
            &app,
            "GET",
            &format!("{HEADQUARTERS_MENUS}{query}"),
            HEADQUARTERS,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}: {body}");
        assert_eq!(body["code"], "validation_failed", "{query}");
        admin.assert_response("get", HEADQUARTERS_MENUS, status, &body);
    }
    let (_, body) = send(
        &app,
        "GET",
        &format!("{HEADQUARTERS_MENUS}{too_long}"),
        HEADQUARTERS,
        None,
    )
    .await;
    assert_eq!(
        body["message"],
        "レッスンメニューの名前の検索語は 40 文字まで"
    );
}

#[tokio::test]
async fn only_headquarters_lists_menus_of_every_coach() {
    let admin = support::Contract::load("admin-web");
    let app = app().await;
    for token in [COACH, STUDENT] {
        let (status, body) = send(&app, "GET", HEADQUARTERS_MENUS, token, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        admin.assert_response("get", HEADQUARTERS_MENUS, status, &body);
    }
    let (status, body) = send(&app, "GET", HEADQUARTERS_MENUS, "nope", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    admin.assert_response("get", HEADQUARTERS_MENUS, status, &body);

    let (status, body) = send(&app, "GET", HEADQUARTERS_MENUS, HEADQUARTERS, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["lesson_menus"], json!([]));
}

#[tokio::test]
async fn a_menu_whose_coach_cannot_be_found_has_no_display_name_and_comes_last() {
    let admin = support::Contract::load("admin-web");
    let orphan = LessonMenu::restore(
        LessonMenuId::from_u128(7),
        AccountId::from_u128(99),
        LessonMenuContent {
            sport: Sport::Tennis,
            format: LessonFormat::Async,
            name: LessonMenuName::new("持ち主のいないメニュー").unwrap(),
            description: LessonMenuDescription::default(),
        },
        true,
        now(),
    );
    let app = app_with(InMemoryLessonMenuRepository::with_menus([orphan])).await;
    send(
        &app,
        "POST",
        ADMIN_MENUS,
        COACH,
        Some(live_menu("tennis", "フォア")),
    )
    .await;

    let (status, body) = send(&app, "GET", HEADQUARTERS_MENUS, HEADQUARTERS, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    admin.assert_response("get", HEADQUARTERS_MENUS, status, &body);
    let menus = body["lesson_menus"].as_array().unwrap();
    assert_eq!(menus.len(), 2, "{body}");
    assert_eq!(menus[0]["coach_display_name"], "たなか");
    assert_eq!(menus[1]["name"], "持ち主のいないメニュー");
    assert!(
        menus[1].get("coach_display_name").is_none(),
        "表示名が引けなければ載せない: {body}"
    );
}

#[tokio::test]
async fn each_menu_has_the_number_of_active_bookings_of_the_last_thirty_days() {
    let admin = support::Contract::load("admin-web");
    let menu = |id: u128, name: &str| {
        LessonMenu::restore(
            LessonMenuId::from_u128(id),
            AccountId::from_u128(42),
            LessonMenuContent {
                sport: Sport::Tennis,
                format: LessonFormat::new(
                    LessonType::Live,
                    Some(LessonDuration::from_minutes(60).unwrap()),
                )
                .unwrap(),
                name: LessonMenuName::new(name).unwrap(),
                description: LessonMenuDescription::default(),
            },
            true,
            now() - Duration::from_secs(60 * DAY),
        )
    };
    let (popular, quiet) = (menu(7, "フォア"), menu(8, "バック"));
    let slots = InMemorySlotRepository::with_slots((1..=3).map(|id| {
        let starts_at = now() + Duration::from_secs(id as u64 * DAY);
        Slot::restore(
            SlotId::from_u128(id),
            AccountId::from_u128(42),
            Sport::Tennis,
            starts_at,
            starts_at + Duration::from_secs(3600),
            SlotStatus::Open,
            SlotSeats::default(),
        )
        .unwrap()
        .with_lesson_menu(&popular)
        .unwrap()
    }));
    let bookings = InMemoryBookingRepository::new(slots);
    // 直近 30 日の 2 件を数え、31 日前の 1 件は数えない
    for (slot, student, days_ago) in [(1, 3, 1), (2, 4, 30), (3, 5, 31)] {
        let student = AccountId::from_u128(student);
        bookings
            .reserve(SlotId::from_u128(slot), student, |slot, existing| {
                Booking::reserve(
                    bookings.next_id(),
                    student,
                    false,
                    slot,
                    existing,
                    now() - Duration::from_secs(days_ago * DAY),
                )
                .map(|booking| booking.with_lesson_menu(slot, Some(&popular)))
            })
            .await
            .unwrap();
    }
    let app = app_with_bookings(
        InMemoryLessonMenuRepository::with_menus([popular.clone(), quiet]),
        bookings,
    )
    .await;

    let (status, body) = send(&app, "GET", HEADQUARTERS_MENUS, HEADQUARTERS, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    admin.assert_response("get", HEADQUARTERS_MENUS, status, &body);
    let counts: Vec<(&str, &Value)> = body["lesson_menus"]
        .as_array()
        .unwrap()
        .iter()
        .map(|menu| {
            (
                menu["name"].as_str().unwrap(),
                &menu["recent_booking_count"],
            )
        })
        .collect();
    assert_eq!(
        counts,
        [("フォア", &json!(2)), ("バック", &json!(0))],
        "予約のないメニューは 0: {body}"
    );
}
