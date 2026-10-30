//! コーチのレッスンメニュー（catalog）の統合テスト。admin-web の `GET` / `POST /coaches/me/lesson-menus`
//! （`listMyLessonMenus` / `createLessonMenu`）と `PUT /coaches/me/lesson-menus/{lesson_menu_id}`（`updateLessonMenu`）、
//! student-app の `GET /coaches/{coach_id}/lesson-menus`（`listCoachLessonMenus`）。レスポンスの形をそれぞれの OpenAPI の
//! スキーマで検査する。
//!
//! 作る・直す・非公開にするのはコーチ本人だけ（生徒と本部は 403、別のコーチのメニューは 404）で、生徒の一覧には公開中の
//! メニューだけが出ること（ゴルフは student-app の範囲外なので出ない）を確かめる。

use std::time::{Duration, SystemTime};

use application::{
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo},
    catalog::{CreateLessonMenu, ListCoachLessonMenus, ListMyLessonMenus, UpdateLessonMenu},
    entitlement::{EntitlementQuotaGate, EntitlementReviewGate, GetEntitlement, StartSubscription},
    identity::{
        AccessToken, AccountRepository, IdentityCoachLookup, InvalidToken, RegisterAccount,
        TokenClaims, TokenIssuer, TokenVerifier,
    },
    in_memory::{
        FixedAiFeedbackGenerator, FixedVideoCallProvider, FixedVideoStorage,
        InMemoryAccountRepository, InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemoryLessonCompletionRepository, InMemoryLessonMenuRepository,
        InMemoryLessonReviewRepository, InMemoryLiveLessonSessionRepository,
        InMemorySlotRepository, InMemoryVideoRepository, ManualClock, RecordingAnalyticsSink,
        RecordingDomainEventPublisher,
    },
    lesson::RecordLessonCompletion,
    live_lesson::{EndLiveLessonSession, StartLiveLessonSession},
    review::{GetCoachReviewSummary, ListCoachReviews, SubmitLessonReview},
    scheduling::{
        CancelBooking, ConfirmBooking, CreateBooking, GetSlotSupply, PublishSlot,
        SchedulingBookingLookup, SearchSlots,
    },
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use domain::{
    identity::{Account, AccountId, Email, Role, Sport},
    profile::DisplayName,
};
use serde_json::{Value, json};
use tower::ServiceExt;

mod support;

/// 2026-10-26T00:00:00Z（最初の現在時刻）
const NOW: Duration = Duration::from_secs(1_792_972_800);

/// `<役割>:<ID の数値>` の形のトークンを通す偽物（interface は infra のモックに依存できない）。
struct FakeTokens;

impl TokenIssuer for FakeTokens {
    fn issue(&self, _claims: TokenClaims) -> AccessToken {
        unreachable!("このテストでは発行しない")
    }
}

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

/// テニスとゴルフを提供するコーチ（ID 42 = 0x2a）と、そのトークン
const COACH_ID: &str = "00000000-0000-0000-0000-00000000002a";
const COACH: &str = "coach:42";
/// サッカーを提供する別のコーチ（ID 43 = 0x2b）
const OTHER_COACH_ID: &str = "00000000-0000-0000-0000-00000000002b";
const OTHER_COACH: &str = "coach:43";
/// アカウントのないコーチのトークン
const UNKNOWN_COACH: &str = "coach:44";
const STUDENT: &str = "student:3";
const HEADQUARTERS: &str = "headquarters:1";

fn at(minutes: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + NOW + Duration::from_secs(minutes * 60)
}

fn account(id: u128, role: Role, sports: &[Sport]) -> Account {
    Account::new(
        AccountId::from_u128(id),
        Email::new(format!("user{id}@example.com")).unwrap(),
        DisplayName::new("テスト").unwrap(),
        role,
        sports.iter().copied(),
        at(0),
    )
    .unwrap()
}

/// コーチ 42・43 と生徒 3・本部 1 のアカウント。
async fn accounts() -> InMemoryAccountRepository {
    let accounts = InMemoryAccountRepository::default();
    for account in [
        account(42, Role::Coach, &[Sport::Tennis, Sport::Golf]),
        account(43, Role::Coach, &[Sport::Soccer]),
        account(3, Role::Student, &[Sport::Tennis]),
        account(1, Role::Headquarters, &[]),
    ] {
        accounts.insert(account).await.unwrap();
    }
    accounts
}

fn app(
    accounts: InMemoryAccountRepository,
    menus: InMemoryLessonMenuRepository,
    clock: ManualClock,
) -> Router {
    let analytics = RecordingAnalyticsSink::default();
    let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::default());
    let slots = bookings.slots().clone();
    interface::router_with(
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            analytics.clone(),
            clock.clone(),
        ),
        application::profile::ListCoaches::new(InMemoryAccountRepository::default()),
        application::identity::GetAccount::new(InMemoryAccountRepository::default()),
        application::profile::UpdateCoachProfile::new(InMemoryAccountRepository::default()),
        CreateLessonMenu::new(
            menus.clone(),
            IdentityCoachLookup::new(accounts),
            clock.clone(),
        ),
        UpdateLessonMenu::new(menus.clone(), clock.clone()),
        ListMyLessonMenus::new(menus.clone()),
        ListCoachLessonMenus::new(menus),
        PublishSlot::new(slots.clone(), analytics.clone(), clock.clone()),
        SearchSlots::new(
            slots,
            application::profile::ProfileCoachIntroductionLookup::new(
                InMemoryAccountRepository::default(),
            ),
            clock.clone(),
        ),
        CreateBooking::new(bookings.clone(), analytics.clone(), clock.clone()),
        ConfirmBooking::new(
            bookings.clone(),
            EntitlementQuotaGate::new(InMemoryEntitlementRepository::default()),
            analytics.clone(),
            RecordingDomainEventPublisher::default(),
            clock.clone(),
        ),
        CancelBooking::new(
            bookings.clone(),
            analytics.clone(),
            RecordingDomainEventPublisher::default(),
            clock.clone(),
        ),
        GetSlotSupply::new(bookings, clock.clone()),
        application::scheduling::ListMyBookings::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            application::profile::ProfileCoachIntroductionLookup::new(
                InMemoryAccountRepository::default(),
            ),
        ),
        application::scheduling::ListCoachBookings::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            application::profile::ProfileStudentDisplayNameLookup::new(
                InMemoryAccountRepository::default(),
            ),
        ),
        application::scheduling::ListCoachSlots::new(
            InMemorySlotRepository::default(),
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
        ),
        StartSubscription::new(
            InMemoryEntitlementRepository::default(),
            analytics.clone(),
            RecordingDomainEventPublisher::default(),
            clock.clone(),
        ),
        GetEntitlement::new(InMemoryEntitlementRepository::default(), clock.clone()),
        StartLiveLessonSession::new(
            InMemoryLiveLessonSessionRepository::default(),
            SchedulingBookingLookup::new(InMemoryBookingRepository::new(
                InMemorySlotRepository::default(),
            )),
            FixedVideoCallProvider::default(),
            analytics.clone(),
            clock.clone(),
        ),
        EndLiveLessonSession::new(
            InMemoryLiveLessonSessionRepository::default(),
            FixedVideoCallProvider::default(),
            analytics.clone(),
            clock.clone(),
        ),
        SubmitVideo::new(
            InMemoryVideoRepository::default(),
            FixedVideoStorage,
            FixedAiFeedbackGenerator,
            analytics.clone(),
            application::in_memory::RecordingDomainEventPublisher::default(),
            clock.clone(),
        ),
        GetVideoReview::new(InMemoryVideoRepository::default()),
        ReturnVideoReview::new(
            InMemoryVideoRepository::default(),
            EntitlementReviewGate,
            analytics.clone(),
            application::in_memory::RecordingDomainEventPublisher::default(),
            clock.clone(),
        ),
        application::async_lesson::ListCoachVideos::new(
            InMemoryVideoRepository::default(),
            application::profile::ProfileStudentDisplayNameLookup::new(
                InMemoryAccountRepository::default(),
            ),
        ),
        RecordLessonCompletion::new(
            InMemoryLiveLessonSessionRepository::default(),
            InMemoryVideoRepository::default(),
            InMemoryLessonCompletionRepository::default(),
            analytics,
            RecordingDomainEventPublisher::default(),
            clock.clone(),
        ),
        application::messaging::ListMessageThreads::new(
            application::in_memory::InMemoryMessageThreadRepository::default(),
        ),
        application::messaging::ListMessages::new(
            application::in_memory::InMemoryMessageThreadRepository::default(),
        ),
        application::messaging::SendMessage::new(
            application::in_memory::InMemoryMessageThreadRepository::default(),
            application::entitlement::EntitlementChatGate::new(
                application::in_memory::InMemoryEntitlementRepository::default(),
            ),
            RecordingAnalyticsSink::default(),
            application::in_memory::RecordingDomainEventPublisher::default(),
            clock.clone(),
        ),
        application::messaging::MarkMessageThreadRead::new(
            application::in_memory::InMemoryMessageThreadRepository::default(),
            clock.clone(),
        ),
        SubmitLessonReview::new(
            application::lesson::LessonCompletionLookup::new(
                application::in_memory::InMemoryLessonCompletionRepository::default(),
            ),
            InMemoryLessonReviewRepository::default(),
            RecordingAnalyticsSink::default(),
            clock.clone(),
        ),
        GetCoachReviewSummary::new(InMemoryLessonReviewRepository::default()),
        ListCoachReviews::new(InMemoryLessonReviewRepository::default()),
        application::notification::GetNotificationSettings::new(
            application::in_memory::InMemoryNotificationSettingsRepository::default(),
        ),
        application::notification::UpdateNotificationSettings::new(
            application::in_memory::InMemoryNotificationSettingsRepository::default(),
        ),
        FakeTokens,
    )
}

struct Fixture {
    router: Router,
    clock: ManualClock,
}

async fn fixture() -> Fixture {
    let clock = ManualClock::new(at(0));
    Fixture {
        router: app(
            accounts().await,
            InMemoryLessonMenuRepository::default(),
            clock.clone(),
        ),
        clock,
    }
}

/// `method path` を送り、レスポンスの形を `app` の契約（`template` の操作）で検査する。
async fn send(
    router: &Router,
    app: &str,
    method: Method,
    path: &str,
    template: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method.clone()).uri(path);
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let request = match body {
        Some(body) => request
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap();
    support::Contract::load(app).assert_response(
        &method.as_str().to_lowercase(),
        template,
        status,
        &body,
    );
    (status, body)
}

async fn create(router: &Router, token: &str, body: Value) -> (StatusCode, Value) {
    send(
        router,
        "admin-web",
        Method::POST,
        "/coaches/me/lesson-menus",
        "/coaches/me/lesson-menus",
        Some(token),
        Some(body),
    )
    .await
}

async fn update(router: &Router, token: &str, id: &str, body: Value) -> (StatusCode, Value) {
    send(
        router,
        "admin-web",
        Method::PUT,
        &format!("/coaches/me/lesson-menus/{id}"),
        "/coaches/me/lesson-menus/{lesson_menu_id}",
        Some(token),
        Some(body),
    )
    .await
}

async fn list_mine(router: &Router, token: &str) -> (StatusCode, Value) {
    send(
        router,
        "admin-web",
        Method::GET,
        "/coaches/me/lesson-menus",
        "/coaches/me/lesson-menus",
        Some(token),
        None,
    )
    .await
}

async fn list_coach(router: &Router, token: &str, query: &str) -> (StatusCode, Value) {
    send(
        router,
        "student-app",
        Method::GET,
        &format!("/coaches/{query}"),
        "/coaches/{coach_id}/lesson-menus",
        Some(token),
        None,
    )
    .await
}

fn live_menu(name: &str) -> Value {
    json!({
        "sport": "tennis",
        "lesson_type": "live_lesson",
        "name": name,
        "description": "フォームを動画で確かめながら直す",
        "duration_minutes": 60,
    })
}

fn names(list: &Value) -> Vec<&str> {
    list["lesson_menus"]
        .as_array()
        .unwrap()
        .iter()
        .map(|menu| menu["name"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn a_coach_creates_live_and_async_menus() {
    let Fixture { router, .. } = fixture().await;

    let (status, menu) = create(&router, COACH, live_menu("フォーム改善")).await;
    assert_eq!(status, StatusCode::CREATED, "{menu}");
    assert_eq!(menu["coach_id"], COACH_ID);
    assert_eq!(menu["sport"], "tennis");
    assert_eq!(menu["lesson_type"], "live_lesson");
    assert_eq!(menu["name"], "フォーム改善");
    assert_eq!(menu["description"], "フォームを動画で確かめながら直す");
    assert_eq!(menu["duration_minutes"], 60);
    assert_eq!(menu["is_published"], true, "省けば公開中");
    assert_eq!(menu["created_at"], "2026-10-26T00:00:00Z");
    assert_eq!(menu["updated_at"], menu["created_at"]);

    // 非同期レッスンは 1 回の時間を持たない。説明を省けば空、null の時間は省いたのと同じ
    let (status, menu) = create(
        &router,
        COACH,
        json!({
            "sport": "golf",
            "lesson_type": "async_lesson",
            "name": "スイング添削",
            "duration_minutes": null,
            "is_published": false,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{menu}");
    assert_eq!(menu["lesson_type"], "async_lesson");
    assert_eq!(menu["duration_minutes"], Value::Null);
    assert_eq!(menu["description"], "");
    assert_eq!(menu["is_published"], false);
}

#[tokio::test]
async fn invalid_menus_are_422_and_save_nothing() {
    let Fixture { router, .. } = fixture().await;

    let long_name = "あ".repeat(41);
    let long_description = "あ".repeat(401);
    for body in [
        // 提供スポーツでない
        json!({"sport": "soccer", "lesson_type": "async_lesson", "name": "添削"}),
        // 同期レッスンに時間がない・非同期レッスンに時間がある
        json!({"sport": "tennis", "lesson_type": "live_lesson", "name": "通話"}),
        json!({"sport": "tennis", "lesson_type": "async_lesson", "name": "添削", "duration_minutes": 30}),
        // 15〜120 の 15 分刻みでない
        json!({"sport": "tennis", "lesson_type": "live_lesson", "name": "通話", "duration_minutes": 50}),
        json!({"sport": "tennis", "lesson_type": "live_lesson", "name": "通話", "duration_minutes": 0}),
        json!({"sport": "tennis", "lesson_type": "live_lesson", "name": "通話", "duration_minutes": 135}),
        // 名前・説明
        json!({"sport": "tennis", "lesson_type": "async_lesson", "name": " "}),
        json!({"sport": "tennis", "lesson_type": "async_lesson", "name": long_name}),
        json!({"sport": "tennis", "lesson_type": "async_lesson", "name": "添削", "description": long_description}),
        // 契約にない値・項目の欠け
        json!({"sport": "cricket", "lesson_type": "async_lesson", "name": "添削"}),
        json!({"sport": "tennis", "lesson_type": "group_lesson", "name": "添削"}),
        json!({"sport": "tennis", "lesson_type": "async_lesson"}),
    ] {
        let (status, error) = create(&router, COACH, body.clone()).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}: {error}");
        assert_eq!(error["code"], "validation_failed");
    }
    let (_, list) = list_mine(&router, COACH).await;
    assert!(names(&list).is_empty());

    // 40 文字ちょうどの名前と 400 文字ちょうどの説明は通る
    let (status, _) = create(
        &router,
        COACH,
        json!({
            "sport": "tennis",
            "lesson_type": "async_lesson",
            "name": "あ".repeat(40),
            "description": "あ".repeat(400),
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn only_coaches_can_create_update_and_list_their_menus() {
    let Fixture { router, .. } = fixture().await;
    let (_, menu) = create(&router, COACH, live_menu("フォーム改善")).await;
    let id = menu["lesson_menu_id"].as_str().unwrap();

    // 生徒と本部は 403（生徒とコーチを取り違えない）
    for token in [STUDENT, HEADQUARTERS] {
        let (status, error) = create(&router, token, live_menu("乗っ取り")).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}");
        assert_eq!(error["code"], "forbidden");
        let (status, _) = update(&router, token, id, live_menu("乗っ取り")).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}");
        let (status, _) = list_mine(&router, token).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}");
    }

    // トークンがなければ 401
    let (status, _) = send(
        &router,
        "admin-web",
        Method::GET,
        "/coaches/me/lesson-menus",
        "/coaches/me/lesson-menus",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // トークンが示すコーチのアカウントがなければ、無効なトークンとして 401
    let (status, error) = create(&router, UNKNOWN_COACH, live_menu("幽霊")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(error["code"], "unauthorized");

    let (_, list) = list_mine(&router, COACH).await;
    assert_eq!(names(&list), ["フォーム改善"], "何も変わらない");
}

#[tokio::test]
async fn the_coach_revises_and_unpublishes_their_own_menu_only() {
    let Fixture { router, clock } = fixture().await;
    let (_, menu) = create(&router, COACH, live_menu("フォーム改善")).await;
    let id = menu["lesson_menu_id"].as_str().unwrap();

    clock.set(at(30));
    let (status, revised) = update(
        &router,
        COACH,
        id,
        json!({
            "name": "サーブ特訓",
            "description": "",
            "duration_minutes": 90,
            "is_published": false,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{revised}");
    assert_eq!(revised["lesson_menu_id"], id);
    assert_eq!(revised["name"], "サーブ特訓");
    assert_eq!(revised["description"], "");
    assert_eq!(revised["duration_minutes"], 90);
    assert_eq!(revised["is_published"], false);
    assert_eq!(revised["sport"], "tennis", "スポーツは変わらない");
    assert_eq!(revised["lesson_type"], "live_lesson", "種類は変わらない");
    assert_eq!(revised["created_at"], "2026-10-26T00:00:00Z");
    assert_eq!(revised["updated_at"], "2026-10-26T00:30:00Z");

    // 別のコーチのメニュー・ないメニュー・UUID でない ID は 404（あることを知らせない）
    for (token, id) in [
        (OTHER_COACH, id),
        (COACH, "00000000-0000-4000-8000-000000000099"),
        (COACH, "not-a-uuid"),
    ] {
        let body = json!({
            "name": "乗っ取り",
            "description": "",
            "duration_minutes": 30,
            "is_published": true,
        });
        let (status, error) = update(&router, token, id, body).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{token} {id}");
        assert_eq!(error["code"], "not_found");
    }

    // 同期レッスンの時間を外す・刻みでない時間は 422
    for body in [
        json!({"name": "時間なし", "description": "", "is_published": true}),
        json!({"name": "半端", "description": "", "duration_minutes": 20, "is_published": true}),
        json!({"name": "", "description": "", "duration_minutes": 30, "is_published": true}),
        json!({"name": "公開の欠け", "description": "", "duration_minutes": 30}),
    ] {
        let (status, error) = update(&router, COACH, id, body.clone()).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}: {error}");
    }

    let (_, list) = list_mine(&router, COACH).await;
    assert_eq!(list["lesson_menus"].as_array().unwrap().len(), 1);
    assert_eq!(
        list["lesson_menus"][0], revised,
        "拒否した変更は何も変えない"
    );
}

#[tokio::test]
async fn the_coach_lists_every_own_menu_oldest_first() {
    let Fixture { router, clock } = fixture().await;
    create(&router, COACH, live_menu("1 本目")).await;
    clock.set(at(1));
    let (_, second) = create(&router, COACH, live_menu("2 本目")).await;
    clock.set(at(2));
    create(
        &router,
        OTHER_COACH,
        json!({"sport": "soccer", "lesson_type": "async_lesson", "name": "別のコーチ"}),
    )
    .await;
    clock.set(at(3));
    update(
        &router,
        COACH,
        second["lesson_menu_id"].as_str().unwrap(),
        json!({"name": "2 本目", "description": "", "duration_minutes": 60, "is_published": false}),
    )
    .await;

    let (status, list) = list_mine(&router, COACH).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["coach_id"], COACH_ID);
    assert_eq!(
        names(&list),
        ["1 本目", "2 本目"],
        "非公開も含み、他のコーチのメニューは含まない"
    );

    let (_, empty) = list_mine(&router, "coach:45").await;
    assert_eq!(empty["lesson_menus"], json!([]));
}

#[tokio::test]
async fn students_see_only_published_menus_of_the_sports_they_handle() {
    let Fixture { router, clock } = fixture().await;
    create(&router, COACH, live_menu("テニス 通話")).await;
    clock.set(at(1));
    create(
        &router,
        COACH,
        json!({"sport": "tennis", "lesson_type": "async_lesson", "name": "テニス 添削", "is_published": false}),
    )
    .await;
    clock.set(at(2));
    create(
        &router,
        COACH,
        json!({"sport": "golf", "lesson_type": "async_lesson", "name": "ゴルフ 添削"}),
    )
    .await;
    clock.set(at(3));
    create(
        &router,
        COACH,
        json!({"sport": "tennis", "lesson_type": "async_lesson", "name": "テニス 添削 2"}),
    )
    .await;

    let (status, list) = list_coach(&router, STUDENT, &format!("{COACH_ID}/lesson-menus")).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list["coach_id"], COACH_ID);
    assert_eq!(
        names(&list),
        ["テニス 通話", "テニス 添削 2"],
        "非公開とゴルフ（student-app の範囲外）は出ない"
    );
    assert!(
        list["lesson_menus"]
            .as_array()
            .unwrap()
            .iter()
            .all(|menu| menu["is_published"] == true)
    );

    let (_, tennis) = list_coach(
        &router,
        STUDENT,
        &format!("{COACH_ID}/lesson-menus?sport=tennis"),
    )
    .await;
    assert_eq!(names(&tennis), ["テニス 通話", "テニス 添削 2"]);
    let (_, soccer) = list_coach(
        &router,
        STUDENT,
        &format!("{COACH_ID}/lesson-menus?sport=soccer"),
    )
    .await;
    assert_eq!(soccer["lesson_menus"], json!([]));

    // メニューのないコーチ・コーチでない ID は空
    let (_, other) = list_coach(&router, STUDENT, &format!("{OTHER_COACH_ID}/lesson-menus")).await;
    assert_eq!(other["lesson_menus"], json!([]));
}

#[tokio::test]
async fn the_student_list_rejects_golf_unknown_sports_bad_ids_and_other_roles() {
    let Fixture { router, .. } = fixture().await;

    for query in ["sport=golf", "sport=cricket"] {
        let (status, error) = list_coach(
            &router,
            STUDENT,
            &format!("{COACH_ID}/lesson-menus?{query}"),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}");
        assert_eq!(error["code"], "validation_failed");
    }
    let (status, _) = list_coach(&router, STUDENT, "not-a-uuid/lesson-menus").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // コーチ・本部は student-app の一覧を呼べない（コーチは admin-web の listMyLessonMenus）
    for token in [COACH, HEADQUARTERS] {
        let (status, error) = list_coach(&router, token, &format!("{COACH_ID}/lesson-menus")).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}");
        assert_eq!(error["code"], "forbidden");
    }
}
