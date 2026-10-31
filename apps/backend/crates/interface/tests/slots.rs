//! `/slots` の統合テスト。`POST /slots`（admin-web の `publishSlot`）と `GET /slots`（student-app の
//! `searchSlots`）のレスポンスの形を、それぞれの OpenAPI のスキーマで検査する。枠に結びつけるレッスンメニュー
//! （`lesson_menu_id`）の 422 と、検索の枠に添えるメニュー（`lesson_menu`）も確かめる。

use std::time::{Duration, SystemTime};

use application::identity::AccountRepository;
use application::{
    analytics::AnalyticsEvent,
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo},
    entitlement::{EntitlementQuotaGate, EntitlementReviewGate, GetEntitlement, StartSubscription},
    identity::{
        AccessToken, InvalidToken, RegisterAccount, TokenClaims, TokenIssuer, TokenVerifier,
        account_id_uuid,
    },
    in_memory::{
        FixedAiFeedbackGenerator, FixedClock, FixedVideoCallProvider, FixedVideoStorage,
        InMemoryAccountRepository, InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemoryLessonCompletionRepository, InMemoryLiveLessonSessionRepository,
        InMemorySlotRepository, InMemoryVideoRepository, RecordingAnalyticsSink,
        RecordingDomainEventPublisher,
    },
    lesson::RecordLessonCompletion,
    live_lesson::{EndLiveLessonSession, StartLiveLessonSession},
    scheduling::{
        CancelBooking, ConfirmBooking, CreateBooking, GetSlotSupply, PublishSlot,
        SchedulingBookingLookup, SearchSlots, slot_id_uuid,
    },
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    identity::{Account, AccountId, Email, Role},
    profile::{CoachBio, DisplayName},
};
use serde_json::{Value, json};
use tower::ServiceExt;

mod support;

/// 2026-10-26T00:00:00Z（現在時刻）
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

const COACH_A: &str = "coach:1";
const COACH_B: &str = "coach:2";
const STUDENT: &str = "student:3";

fn coach_id(token: &str) -> String {
    let id = token.split_once(':').unwrap().1.parse().unwrap();
    account_id_uuid::format(AccountId::from_u128(id))
}

struct App {
    router: Router,
    analytics: RecordingAnalyticsSink,
    /// `GET /slots` がコーチの紹介を引くアカウントの保存先
    accounts: InMemoryAccountRepository,
    /// `POST /slots` と `GET /slots` が枠に結びついたレッスンメニューを引く保存先
    lesson_menus: application::in_memory::InMemoryLessonMenuRepository,
}

fn app() -> App {
    let analytics = RecordingAnalyticsSink::default();
    let clock = FixedClock(SystemTime::UNIX_EPOCH + NOW);
    let accounts = InMemoryAccountRepository::default();
    let slots = InMemorySlotRepository::default();
    let bookings = InMemoryBookingRepository::new(slots.clone());
    let lesson_menus = application::in_memory::InMemoryLessonMenuRepository::default();
    let router = interface::router_with(
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            analytics.clone(),
            clock,
        ),
        application::identity::GetAccount::new(InMemoryAccountRepository::default()),
        application::profile::UpdateCoachProfile::new(InMemoryAccountRepository::default()),
        PublishSlot::new(
            slots.clone(),
            application::catalog::CatalogLessonMenuLookup::new(lesson_menus.clone()),
            analytics.clone(),
            clock,
        ),
        SearchSlots::new(
            slots,
            application::catalog::CatalogLessonMenuLookup::new(lesson_menus.clone()),
            application::profile::ProfileCoachIntroductionLookup::new(accounts.clone()),
            clock,
        ),
        CreateBooking::new(
            bookings.clone(),
            application::catalog::CatalogLessonMenuLookup::new(lesson_menus.clone()),
            analytics.clone(),
            clock,
        ),
        ConfirmBooking::new(
            bookings.clone(),
            EntitlementQuotaGate::new(InMemoryEntitlementRepository::default()),
            analytics.clone(),
            RecordingDomainEventPublisher::default(),
            clock,
        ),
        CancelBooking::new(
            bookings,
            analytics.clone(),
            RecordingDomainEventPublisher::default(),
            clock,
        ),
        GetSlotSupply::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            clock,
        ),
        application::scheduling::ListMyBookings::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            application::profile::ProfileCoachIntroductionLookup::new(
                InMemoryAccountRepository::default(),
            ),
            EntitlementQuotaGate::new(InMemoryEntitlementRepository::default()),
            clock,
        ),
        application::scheduling::ListCoachBookings::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            application::profile::ProfileStudentDisplayNameLookup::new(
                InMemoryAccountRepository::default(),
            ),
        ),
        application::scheduling::ListCoachSlots::new(
            InMemorySlotRepository::default(),
            application::catalog::CatalogLessonMenuLookup::new(lesson_menus.clone()),
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
        ),
        StartSubscription::new(
            InMemoryEntitlementRepository::default(),
            analytics.clone(),
            RecordingDomainEventPublisher::default(),
            clock,
        ),
        GetEntitlement::new(InMemoryEntitlementRepository::default(), clock),
        StartLiveLessonSession::new(
            InMemoryLiveLessonSessionRepository::default(),
            SchedulingBookingLookup::new(InMemoryBookingRepository::new(
                InMemorySlotRepository::default(),
            )),
            FixedVideoCallProvider::default(),
            RecordingAnalyticsSink::default(),
            clock,
        ),
        EndLiveLessonSession::new(
            InMemoryLiveLessonSessionRepository::default(),
            FixedVideoCallProvider::default(),
            RecordingAnalyticsSink::default(),
            clock,
        ),
        SubmitVideo::new(
            InMemoryVideoRepository::default(),
            application::catalog::CatalogLessonMenuLookup::new(
                application::in_memory::InMemoryLessonMenuRepository::default(),
            ),
            FixedVideoStorage,
            FixedAiFeedbackGenerator,
            RecordingAnalyticsSink::default(),
            application::in_memory::RecordingDomainEventPublisher::default(),
            clock,
        ),
        GetVideoReview::new(InMemoryVideoRepository::default()),
        ReturnVideoReview::new(
            InMemoryVideoRepository::default(),
            EntitlementReviewGate,
            RecordingAnalyticsSink::default(),
            application::in_memory::RecordingDomainEventPublisher::default(),
            clock,
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
            RecordingAnalyticsSink::default(),
            RecordingDomainEventPublisher::default(),
            clock,
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
            clock,
        ),
        application::messaging::MarkMessageThreadRead::new(
            application::in_memory::InMemoryMessageThreadRepository::default(),
            clock,
        ),
        application::review::SubmitLessonReview::new(
            application::lesson::LessonCompletionLookup::new(
                application::in_memory::InMemoryLessonCompletionRepository::default(),
            ),
            application::in_memory::InMemoryLessonReviewRepository::default(),
            RecordingAnalyticsSink::default(),
            clock,
        ),
        application::review::GetCoachReviewSummary::new(
            application::in_memory::InMemoryLessonReviewRepository::default(),
        ),
        application::review::ListCoachReviews::new(
            application::in_memory::InMemoryLessonReviewRepository::default(),
            application::lesson::LessonCompletionMenuLookup::new(
                application::in_memory::InMemoryLessonCompletionRepository::default(),
                application::in_memory::InMemoryLiveLessonSessionRepository::default(),
                application::scheduling::SchedulingBookingLookup::new(
                    application::in_memory::InMemoryBookingRepository::new(
                        application::in_memory::InMemorySlotRepository::default(),
                    ),
                ),
                application::in_memory::InMemoryVideoRepository::default(),
            ),
        ),
        application::notification::GetNotificationSettings::new(
            application::in_memory::InMemoryNotificationSettingsRepository::default(),
        ),
        application::notification::UpdateNotificationSettings::new(
            application::in_memory::InMemoryNotificationSettingsRepository::default(),
        ),
        FakeTokens,
    );
    App {
        router,
        analytics,
        accounts,
        lesson_menus,
    }
}

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn post_slots(router: &Router, token: Option<&str>, body: Value) -> (StatusCode, Value) {
    let mut request = Request::post("/slots").header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    send(router, request.body(Body::from(body.to_string())).unwrap()).await
}

async fn get_slots(router: &Router, token: Option<&str>, query: &str) -> (StatusCode, Value) {
    let mut request = Request::get(format!("/slots?{query}"));
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    send(router, request.body(Body::empty()).unwrap()).await
}

fn slot(sport: &str, starts_at: &str, ends_at: &str) -> Value {
    json!({ "sport": sport, "starts_at": starts_at, "ends_at": ends_at })
}

/// 2026-11-01 の `hour` 時（UTC）。
fn nov1(hour: u32) -> String {
    format!("2026-11-01T{hour:02}:00:00Z")
}

/// コーチ `token` の枠を公開し、slot_id を返す。
async fn publish(router: &Router, token: &str, sport: &str, hour: u32) -> String {
    let (status, body) = post_slots(
        router,
        Some(token),
        slot(sport, &nov1(hour), &nov1(hour + 1)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["slot_id"].as_str().unwrap().to_string()
}

fn assert_admin_web(status: StatusCode, body: &Value) {
    support::Contract::load("admin-web").assert_response("post", "/slots", status, body);
}

fn assert_student_app(status: StatusCode, body: &Value) {
    support::Contract::load("student-app").assert_response("get", "/slots", status, body);
}

fn item_ids(body: &Value) -> Vec<String> {
    body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["slot_id"].as_str().unwrap().to_string())
        .collect()
}

// --- POST /slots（admin-web） ---

#[tokio::test]
async fn coach_publishes_a_slot_with_the_admin_web_contract_shape() {
    let app = app();

    let (status, body) = post_slots(
        &app.router,
        Some(COACH_A),
        slot(
            "golf",
            "2026-11-01T19:00:00+09:00",
            "2026-11-01T20:00:00+09:00",
        ),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_admin_web(status, &body);
    assert_eq!(
        body["coach_id"],
        coach_id(COACH_A),
        "コーチはトークンのアカウント"
    );
    assert_eq!(body["sport"], "golf");
    assert_eq!(body["starts_at"], "2026-11-01T10:00:00Z");
    assert_eq!(body["ends_at"], "2026-11-01T11:00:00Z");
    assert_eq!(body["status"], "open");
    assert_eq!(body["capacity"], 1, "定員を省けばマンツーマン");
    assert_eq!(body["remaining_seats"], 1);

    let events = app.analytics.events();
    assert_eq!(events.len(), 1, "slot.published は 1 回だけ発火する");
    assert_eq!(events[0].name(), "slot.published");
    let AnalyticsEvent::SlotPublished {
        coach_id: event_coach,
        slot_id,
        starts_at,
        occurred_at,
    } = &events[0]
    else {
        panic!("{:?}", events[0]);
    };
    assert_eq!(account_id_uuid::format(*event_coach), coach_id(COACH_A));
    assert_eq!(slot_id_uuid::format(*slot_id), body["slot_id"]);
    assert_eq!(
        *starts_at,
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_527_200)
    );
    assert_eq!(*occurred_at, SystemTime::UNIX_EPOCH + NOW);
}

/// メニュー `id`（2026-11-01 に作った名前「フォア改善」のメニュー）を `app` の保存先に入れる。
async fn add_menu(
    app: &App,
    id: u128,
    coach: &str,
    sport: Sport,
    format: domain::catalog::LessonFormat,
    is_published: bool,
) -> String {
    use application::catalog::{LessonMenuRepository, lesson_menu_id_uuid};
    use domain::catalog::{
        LessonMenu, LessonMenuContent, LessonMenuDescription, LessonMenuId, LessonMenuName,
    };

    let coach = AccountId::from_u128(coach.split_once(':').unwrap().1.parse().unwrap());
    let id = LessonMenuId::from_u128(id);
    app.lesson_menus
        .insert(LessonMenu::restore(
            id,
            coach,
            LessonMenuContent {
                sport,
                format,
                name: LessonMenuName::new("フォア改善").unwrap(),
                description: LessonMenuDescription::default(),
            },
            is_published,
            SystemTime::UNIX_EPOCH + NOW,
        ))
        .await
        .unwrap();
    lesson_menu_id_uuid::format(id)
}

fn live(minutes: i64) -> domain::catalog::LessonFormat {
    domain::catalog::LessonFormat::Live(
        domain::catalog::LessonDuration::from_minutes(minutes).unwrap(),
    )
}

#[tokio::test]
async fn coach_publishes_a_slot_bound_to_a_lesson_menu_and_the_search_carries_it() {
    let app = app();
    let menu_id = add_menu(&app, 50, COACH_A, Sport::Tennis, live(60), true).await;
    let mut request = slot("tennis", &nov1(10), &nov1(11));
    request["lesson_menu_id"] = json!(menu_id);

    let (status, body) = post_slots(&app.router, Some(COACH_A), request).await;

    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_admin_web(status, &body);
    assert!(
        body.get("lesson_menu").is_none(),
        "公開の応答には載せない: {body}"
    );
    assert_eq!(app.analytics.events().len(), 1);
    let bound = body["slot_id"].as_str().unwrap().to_string();
    let plain = publish(&app.router, COACH_A, "tennis", 12).await;

    let (status, body) = get_slots(&app.router, Some(STUDENT), "sport=tennis").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_student_app(status, &body);
    assert_eq!(item_ids(&body), [bound, plain]);
    assert_eq!(
        body["items"][0]["lesson_menu"],
        json!({
            "lesson_menu_id": menu_id,
            "name": "フォア改善",
            "duration_minutes": 60,
        })
    );
    assert_eq!(
        body["items"][1]["lesson_menu"],
        Value::Null,
        "メニューのない枠は null"
    );
}

#[tokio::test]
async fn lesson_menu_that_does_not_fit_the_slot_is_422_validation_failed() {
    let app = app();
    let other_coach = add_menu(&app, 50, COACH_B, Sport::Tennis, live(60), true).await;
    let unpublished = add_menu(&app, 51, COACH_A, Sport::Tennis, live(60), false).await;
    let async_menu = add_menu(
        &app,
        52,
        COACH_A,
        Sport::Tennis,
        domain::catalog::LessonFormat::Async,
        true,
    )
    .await;
    let soccer = add_menu(&app, 53, COACH_A, Sport::Soccer, live(60), true).await;
    let ninety = add_menu(&app, 54, COACH_A, Sport::Tennis, live(90), true).await;
    let cases = [
        (other_coach, "自分のレッスンメニューだけ"),
        (unpublished, "公開中でないレッスンメニュー"),
        (async_menu, "非同期レッスンのメニュー"),
        (
            soccer,
            "レッスンメニューのスポーツ（soccer）が枠のスポーツ（tennis）と違う",
        ),
        (ninety, "1 回の時間（90 分）と違う"),
        (
            "00000000-0000-0000-0000-000000000063".to_string(),
            "指定したレッスンメニューがない",
        ),
        ("not-a-uuid".to_string(), "lesson_menu_id が UUID でない"),
    ];
    for (menu_id, message) in cases {
        let mut request = slot("tennis", &nov1(10), &nov1(11));
        request["lesson_menu_id"] = json!(menu_id);
        let (status, body) = post_slots(&app.router, Some(COACH_A), request).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{menu_id}: {body}"
        );
        assert_admin_web(status, &body);
        assert_eq!(body["code"], "validation_failed");
        assert!(
            body["message"].as_str().unwrap().contains(message),
            "{menu_id}: {body}"
        );
    }
    assert!(app.analytics.events().is_empty(), "拒否では発火しない");
    let (_, body) = get_slots(&app.router, Some(STUDENT), "sport=tennis").await;
    assert!(item_ids(&body).is_empty(), "拒否した枠は保存しない");
}

#[tokio::test]
async fn coach_publishes_a_group_slot_with_a_capacity() {
    let app = app();
    let mut request = slot("tennis", &nov1(10), &nov1(11));
    request["capacity"] = json!(3);

    let (status, body) = post_slots(&app.router, Some(COACH_A), request).await;

    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_admin_web(status, &body);
    assert_eq!(body["capacity"], 3);
    assert_eq!(body["remaining_seats"], 3);

    // 生徒の検索にも定員と残りの席が出る
    let (status, body) = get_slots(&app.router, Some(STUDENT), "sport=tennis").await;
    assert_student_app(status, &body);
    assert_eq!(body["items"][0]["capacity"], 3);
    assert_eq!(body["items"][0]["remaining_seats"], 3);
}

#[tokio::test]
async fn overlapping_slot_is_409_slot_overlaps_without_emitting() {
    let app = app();
    let first = publish(&app.router, COACH_A, "tennis", 10).await;

    for (from, to) in [(10, 11), (9, 11), (10, 12), (9, 12)] {
        let (status, body) = post_slots(
            &app.router,
            Some(COACH_A),
            slot("soccer", &nov1(from), &nov1(to)),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{from}-{to}: {body}");
        assert_admin_web(status, &body);
        assert_eq!(body["code"], "slot_overlaps");
        assert!(body["message"].as_str().unwrap().contains(&first));
    }
    assert_eq!(app.analytics.events().len(), 1);

    // 隣り合う枠と、別のコーチの同じ時間の枠は公開できる
    publish(&app.router, COACH_A, "tennis", 11).await;
    publish(&app.router, COACH_B, "tennis", 10).await;
    assert_eq!(app.analytics.events().len(), 3);
}

#[tokio::test]
async fn invalid_publish_request_is_422_validation_failed() {
    let app = app();
    let cases = [
        ("開始と終了が同じ", slot("tennis", &nov1(10), &nov1(10))),
        ("終了が開始より前", slot("tennis", &nov1(11), &nov1(10))),
        ("未知のスポーツ", slot("cricket", &nov1(10), &nov1(11))),
        ("日時の形", slot("tennis", "2026-11-01 10:00", &nov1(11))),
        (
            "必須項目の欠落",
            json!({ "sport": "tennis", "starts_at": nov1(10) }),
        ),
        (
            "定員が 0",
            json!({ "sport": "tennis", "starts_at": nov1(10), "ends_at": nov1(11), "capacity": 0 }),
        ),
        (
            "定員が 7",
            json!({ "sport": "tennis", "starts_at": nov1(10), "ends_at": nov1(11), "capacity": 7 }),
        ),
        (
            "定員が整数でない",
            json!({ "sport": "tennis", "starts_at": nov1(10), "ends_at": nov1(11), "capacity": "3" }),
        ),
    ];
    for (case, request) in cases {
        let (status, body) = post_slots(&app.router, Some(COACH_A), request).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{case}: {body}");
        assert_admin_web(status, &body);
        assert_eq!(body["code"], "validation_failed", "{case}");
    }
    assert!(app.analytics.events().is_empty());
}

#[tokio::test]
async fn only_coaches_can_publish() {
    let app = app();
    for token in [STUDENT, "headquarters:4"] {
        let (status, body) = post_slots(
            &app.router,
            Some(token),
            slot("tennis", &nov1(10), &nov1(11)),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_admin_web(status, &body);
        assert_eq!(body["code"], "forbidden");
    }
    let (status, body) = post_slots(&app.router, None, slot("tennis", &nov1(10), &nov1(11))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_admin_web(status, &body);
    assert!(app.analytics.events().is_empty());
}

// --- GET /slots（student-app） ---

#[tokio::test]
async fn search_returns_open_slots_of_the_sport_with_the_student_app_contract_shape() {
    let app = app();
    let t11 = publish(&app.router, COACH_A, "tennis", 11).await;
    let t10 = publish(&app.router, COACH_B, "tennis", 10).await;
    publish(&app.router, COACH_A, "soccer", 12).await;
    publish(&app.router, COACH_A, "golf", 13).await;

    let (status, body) = get_slots(&app.router, Some(STUDENT), "sport=tennis").await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_student_app(status, &body);
    assert_eq!(item_ids(&body), [t10.clone(), t11], "開始時刻の順");
    assert_eq!(body["next_cursor"], Value::Null);
    let first = &body["items"][0];
    assert_eq!(first["coach_id"], coach_id(COACH_B));
    assert_eq!(first["sport"], "tennis");
    assert_eq!(first["starts_at"], nov1(10));
    assert_eq!(first["ends_at"], nov1(11));
    assert_eq!(first["capacity"], 1);
    assert_eq!(first["remaining_seats"], 1);

    // コーチも検索できる（認証した利用者なら誰でも）
    let (status, _) = get_slots(&app.router, Some(COACH_A), "sport=soccer").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn search_filters_by_coach_and_period() {
    let app = app();
    let a10 = publish(&app.router, COACH_A, "tennis", 10).await;
    let a12 = publish(&app.router, COACH_A, "tennis", 12).await;
    let b11 = publish(&app.router, COACH_B, "tennis", 11).await;

    let (_, body) = get_slots(&app.router, Some(STUDENT), "sport=tennis").await;
    assert_eq!(item_ids(&body), [a10, b11.clone(), a12.clone()]);

    let (_, body) = get_slots(
        &app.router,
        Some(STUDENT),
        &format!("sport=tennis&coach_id={}", coach_id(COACH_B)),
    )
    .await;
    assert_eq!(item_ids(&body), std::slice::from_ref(&b11));

    let (status, body) = get_slots(
        &app.router,
        Some(STUDENT),
        &format!(
            "sport=tennis&starts_after={}&starts_before={}",
            nov1(11),
            nov1(12)
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(item_ids(&body), [b11], "開始は含み、終わりは含まない");

    let (_, body) = get_slots(
        &app.router,
        Some(STUDENT),
        &format!("sport=tennis&starts_after={}", nov1(12)),
    )
    .await;
    assert_eq!(item_ids(&body), [a12]);
}

#[tokio::test]
async fn search_by_coach_combines_with_the_sport_period_and_paging() {
    let app = app();
    let a10 = publish(&app.router, COACH_A, "tennis", 10).await;
    let a12 = publish(&app.router, COACH_A, "tennis", 12).await;
    let a14 = publish(&app.router, COACH_A, "tennis", 14).await;
    let a_soccer = publish(&app.router, COACH_A, "soccer", 16).await;
    publish(&app.router, COACH_B, "tennis", 11).await;
    publish(&app.router, COACH_B, "tennis", 13).await;
    let coach_a = coach_id(COACH_A);

    let (status, body) = get_slots(
        &app.router,
        Some(STUDENT),
        &format!(
            "sport=tennis&coach_id={coach_a}&starts_after={}&starts_before={}",
            nov1(11),
            nov1(14)
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_student_app(status, &body);
    assert_eq!(
        item_ids(&body),
        std::slice::from_ref(&a12),
        "コーチと期間の両方で絞る"
    );
    assert_eq!(body["items"][0]["coach_id"], coach_a);

    let (_, body) = get_slots(
        &app.router,
        Some(STUDENT),
        &format!("sport=soccer&coach_id={coach_a}"),
    )
    .await;
    assert_eq!(item_ids(&body), [a_soccer], "コーチとスポーツの両方で絞る");

    // コーチで絞ったままページを送る（別のコーチの枠は混ざらない）
    let mut seen = Vec::new();
    let mut query = format!("sport=tennis&coach_id={coach_a}&limit=2");
    loop {
        let (status, body) = get_slots(&app.router, Some(STUDENT), &query).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_student_app(status, &body);
        seen.extend(item_ids(&body));
        match body["next_cursor"].as_str() {
            Some(cursor) => {
                query = format!("sport=tennis&coach_id={coach_a}&limit=2&cursor={cursor}")
            }
            None => break,
        }
    }
    assert_eq!(seen, [a10, a12, a14]);
}

#[tokio::test]
async fn search_by_a_coach_without_slots_is_an_empty_page_not_404() {
    let app = app();
    publish(&app.router, COACH_A, "tennis", 10).await;
    // 生徒の ID・ない ID・枠を公開していないコーチ
    for id in [coach_id(STUDENT), coach_id("coach:99"), coach_id(COACH_B)] {
        let (status, body) = get_slots(
            &app.router,
            Some(STUDENT),
            &format!("sport=tennis&coach_id={id}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{id}: {body}");
        assert_student_app(status, &body);
        assert!(item_ids(&body).is_empty(), "{id}");
        assert_eq!(body["next_cursor"], Value::Null);
    }
}

#[tokio::test]
async fn search_starts_from_now_by_default() {
    let app = app();
    // 現在時刻（2026-10-26T00:00:00Z）より前の枠は既定では出ない
    let (status, _) = post_slots(
        &app.router,
        Some(COACH_A),
        slot("tennis", "2026-10-25T10:00:00Z", "2026-10-25T11:00:00Z"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let future = publish(&app.router, COACH_A, "tennis", 10).await;

    let (_, body) = get_slots(&app.router, Some(STUDENT), "sport=tennis").await;
    assert_eq!(item_ids(&body), [future]);
}

#[tokio::test]
async fn search_pages_through_results_with_next_cursor() {
    let app = app();
    let mut expected = Vec::new();
    for hour in 10..15 {
        expected.push(publish(&app.router, COACH_A, "tennis", hour).await);
    }

    let mut seen = Vec::new();
    let mut query = "sport=tennis&limit=2".to_string();
    let mut pages = 0;
    loop {
        let (status, body) = get_slots(&app.router, Some(STUDENT), &query).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_student_app(status, &body);
        pages += 1;
        let ids = item_ids(&body);
        assert!(ids.len() <= 2);
        seen.extend(ids);
        match body["next_cursor"].as_str() {
            Some(cursor) => query = format!("sport=tennis&limit=2&cursor={cursor}"),
            None => break,
        }
    }
    assert_eq!(pages, 3);
    assert_eq!(seen, expected);

    // limit の既定は 20
    let (_, body) = get_slots(&app.router, Some(STUDENT), "sport=tennis").await;
    assert_eq!(item_ids(&body).len(), 5);
}

#[tokio::test]
async fn invalid_search_query_is_422_validation_failed() {
    let app = app();
    for query in [
        "",
        "sport=golf",
        "sport=cricket",
        "sport=tennis&limit=0",
        "sport=tennis&limit=101",
        "sport=tennis&limit=abc",
        "sport=tennis&coach_id=not-a-uuid",
        "sport=tennis&starts_after=yesterday",
        "sport=tennis&starts_before=2026-11-01",
        "sport=tennis&cursor=broken",
    ] {
        let (status, body) = get_slots(&app.router, Some(STUDENT), query).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{query:?}: {body}"
        );
        assert_student_app(status, &body);
        assert_eq!(body["code"], "validation_failed", "{query:?}");
    }
    let (status, _) = get_slots(&app.router, Some(STUDENT), "sport=tennis&limit=100").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn search_without_a_valid_token_is_401() {
    let app = app();
    for token in [None, Some("unknown")] {
        let (status, body) = get_slots(&app.router, token, "sport=tennis").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}: {body}");
        assert_student_app(status, &body);
    }
}

/// コーチ `token` のアカウントを保存する（契約にコーチの登録の操作がないので、保存先に直に入れる）。
async fn register_coach(app: &App, token: &str, display_name: &str, bio: &str) {
    let id = AccountId::from_u128(token.split_once(':').unwrap().1.parse().unwrap());
    let account = Account::new(
        id,
        Email::new(format!("{display_name}@example.com")).unwrap(),
        DisplayName::new(display_name).unwrap(),
        Role::Coach,
        [Sport::Tennis],
        SystemTime::UNIX_EPOCH,
    )
    .unwrap();
    app.accounts.insert(account).await.unwrap();
    app.accounts
        .update_coach_bio(id, CoachBio::new(bio).unwrap())
        .await
        .unwrap();
}

#[tokio::test]
async fn search_adds_the_coach_display_name_and_bio() {
    let app = app();
    register_coach(&app, COACH_A, "たなか", "テニス歴 20 年").await;
    // 自己紹介文を書いていないコーチ
    register_coach(&app, COACH_B, "いとう", "").await;
    let a10 = publish(&app.router, COACH_A, "tennis", 10).await;
    let b11 = publish(&app.router, COACH_B, "tennis", 11).await;
    // アカウントの見つからないコーチ
    let unknown = publish(&app.router, "coach:9", "tennis", 12).await;

    let (status, body) = get_slots(&app.router, Some(STUDENT), "sport=tennis").await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_student_app(status, &body);
    assert_eq!(item_ids(&body), [a10, b11, unknown]);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items[0]["coach_display_name"], "たなか");
    assert_eq!(items[0]["coach_bio"], "テニス歴 20 年");
    assert_eq!(items[1]["coach_display_name"], "いとう");
    assert!(
        !items[1].as_object().unwrap().contains_key("coach_bio"),
        "自己紹介文がないコーチは項目なし: {}",
        items[1]
    );
    for key in ["coach_display_name", "coach_bio"] {
        assert!(
            !items[2].as_object().unwrap().contains_key(key),
            "紹介が引けないコーチは {key} なし: {}",
            items[2]
        );
    }
}
