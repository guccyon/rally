//! `/videos` の統合テスト。student-app の `POST /videos`（`submitVideo`）・`GET /videos/{video_id}/review`
//! （`getVideoReview`）と admin-web の `POST /videos/{video_id}/review`（`returnVideoReview`）のレスポンスの形を
//! OpenAPI のスキーマで検査し、送付 → 返却の一連・返却前の取得が未返却（`in_progress`）になること・行動イベント
//! `video.submitted` / `video_review.returned`（`submitted_at` 付き）・ゴルフの動画の添削に着手できないこと・
//! 役割と当事者による 403 / 404 を確かめる。admin-web の `GET /coaches/me/videos`（`listMyCoachVideos`）は、コーチ本人に
//! 送られた動画だけを添削待ち（送られた時刻の古い順）→ 返却済み（返却の新しい順）に返し、コーチ以外は 403 になることを確かめる。
//! AI の一次フィードバック（`ai_feedback`）は送付の直後に付き、添削とは別の項目として返却の後も変わらないこと、
//! AI が作れなかった動画は null になることを `getVideoReview` と `listMyCoachVideos` で確かめる。
//! 送付で選んだ非同期のレッスンメニュー（`lesson_menu_id`）は送った時点の ID・名前の写し（`lesson_menu`）として
//! `submitVideo`・`getVideoReview`・`returnVideoReview`・`listMyCoachVideos` に載り、後でメニューを直しても変わらないこと、
//! 選ばなければ null・選べないメニューなら 422 で何も受け付けないことを確かめる。

use std::time::{Duration, SystemTime};

use application::{
    analytics::AnalyticsEvent,
    async_lesson::{
        GetVideoReview, ReturnVideoReview, SubmitVideo, video_id_uuid, video_review_id_uuid,
    },
    catalog::{LessonMenuRepository, lesson_menu_id_uuid},
    domain_event::DomainEvent,
    entitlement::{EntitlementQuotaGate, EntitlementReviewGate, GetEntitlement, StartSubscription},
    identity::{
        AccessToken, AccountRepository, InvalidToken, RegisterAccount, TokenClaims, TokenIssuer,
        TokenVerifier, account_id_uuid,
    },
    in_memory::{
        FixedAiFeedbackGenerator, FixedVideoCallProvider, FixedVideoStorage,
        InMemoryAccountRepository, InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemoryLessonCompletionRepository, InMemoryLessonMenuRepository,
        InMemoryLiveLessonSessionRepository, InMemorySlotRepository, InMemoryVideoRepository,
        ManualClock, RecordingAnalyticsSink, RecordingDomainEventPublisher,
    },
    lesson::RecordLessonCompletion,
    live_lesson::{EndLiveLessonSession, StartLiveLessonSession},
    scheduling::{
        CancelBooking, ConfirmBooking, CreateBooking, GetSlotSupply, PublishSlot,
        SchedulingBookingLookup, SearchSlots,
    },
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    async_lesson::{Video, VideoId},
    catalog::{
        LessonDuration, LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription,
        LessonMenuId, LessonMenuName,
    },
    identity::{Account, AccountId, Email, Role, Sport},
    profile::DisplayName,
};
use serde_json::{Value, json};
use tower::ServiceExt;

mod support;

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

const STUDENT: &str = "student:1";
const OTHER_STUDENT: &str = "student:2";
const COACH: &str = "coach:9";
const OTHER_COACH: &str = "coach:8";
const HEADQUARTERS: &str = "headquarters:5";

/// 先に置いておくゴルフの動画（golf-app の範囲なので student-app からは送れない）
const GOLF_VIDEO: VideoId = VideoId::from_u128(77);

/// 先に置いておく、別のコーチ（`OTHER_COACH`）に送られた動画
const OTHER_COACH_VIDEO: VideoId = VideoId::from_u128(78);

fn account_id(token: &str) -> AccountId {
    AccountId::from_u128(token.split_once(':').unwrap().1.parse().unwrap())
}

/// 2026-10-26T10:00:00Z（動画を送る時刻）
fn submitted_at() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800 + 10 * 3600)
}

/// `COACH` のテニスの公開中の非同期のメニュー
const MENU: LessonMenuId = LessonMenuId::from_u128(50);
/// `COACH` のテニスの非公開の非同期のメニュー
const UNPUBLISHED_MENU: LessonMenuId = LessonMenuId::from_u128(51);
/// `COACH` のテニスの公開中の 60 分の同期のメニュー
const LIVE_MENU: LessonMenuId = LessonMenuId::from_u128(52);
/// `OTHER_COACH` のテニスの公開中の非同期のメニュー
const OTHER_COACH_MENU: LessonMenuId = LessonMenuId::from_u128(53);
/// `COACH` のサッカーの公開中の非同期のメニュー
const SOCCER_MENU: LessonMenuId = LessonMenuId::from_u128(54);

fn menu(
    id: LessonMenuId,
    coach: &str,
    sport: Sport,
    format: LessonFormat,
    name: &str,
    is_published: bool,
) -> LessonMenu {
    LessonMenu::restore(
        id,
        account_id(coach),
        LessonMenuContent {
            sport,
            format,
            name: LessonMenuName::new(name).unwrap(),
            description: LessonMenuDescription::default(),
        },
        is_published,
        SystemTime::UNIX_EPOCH,
    )
}

fn lesson_menus() -> InMemoryLessonMenuRepository {
    let sixty = LessonFormat::Live(LessonDuration::from_minutes(60).unwrap());
    InMemoryLessonMenuRepository::with_menus([
        menu(
            MENU,
            COACH,
            Sport::Tennis,
            LessonFormat::Async,
            "フォーム添削",
            true,
        ),
        menu(
            UNPUBLISHED_MENU,
            COACH,
            Sport::Tennis,
            LessonFormat::Async,
            "休止中の添削",
            false,
        ),
        menu(LIVE_MENU, COACH, Sport::Tennis, sixty, "フォア改善", true),
        menu(
            OTHER_COACH_MENU,
            OTHER_COACH,
            Sport::Tennis,
            LessonFormat::Async,
            "別のコーチの添削",
            true,
        ),
        menu(
            SOCCER_MENU,
            COACH,
            Sport::Soccer,
            LessonFormat::Async,
            "シュート添削",
            true,
        ),
    ])
}

struct App {
    accounts: InMemoryAccountRepository,
    lesson_menus: InMemoryLessonMenuRepository,
    router: Router,
    clock: ManualClock,
    analytics: RecordingAnalyticsSink,
    events: RecordingDomainEventPublisher,
}

/// 生徒 1 がコーチ 9 に送ったゴルフの動画 77 と、生徒 1 がコーチ 8 に送ったテニスの動画 78 を置く。アカウントは
/// 置かない（生徒の表示名を引くテストが [`App::student_named`] で置く）。現在時刻は 2026-10-26 10:00。
fn app() -> App {
    let analytics = RecordingAnalyticsSink::default();
    let events = RecordingDomainEventPublisher::default();
    let clock = ManualClock::new(submitted_at());
    let slots = InMemorySlotRepository::default();
    let bookings = InMemoryBookingRepository::new(slots.clone());
    let entitlements = InMemoryEntitlementRepository::default();
    let accounts = InMemoryAccountRepository::default();
    let lesson_menus = lesson_menus();
    let videos = InMemoryVideoRepository::with_videos([
        Video::submit(
            GOLF_VIDEO,
            account_id(STUDENT),
            account_id(COACH),
            Sport::Golf,
            submitted_at(),
        ),
        Video::submit(
            OTHER_COACH_VIDEO,
            account_id(STUDENT),
            account_id(OTHER_COACH),
            Sport::Tennis,
            submitted_at(),
        ),
    ]);
    let router = interface::router_with(
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            analytics.clone(),
            clock.clone(),
        ),
        application::profile::ListCoaches::new(InMemoryAccountRepository::default()),
        application::identity::GetAccount::new(InMemoryAccountRepository::default()),
        application::profile::UpdateCoachProfile::new(InMemoryAccountRepository::default()),
        PublishSlot::new(
            slots.clone(),
            application::catalog::CatalogLessonMenuLookup::new(
                application::in_memory::InMemoryLessonMenuRepository::default(),
            ),
            analytics.clone(),
            clock.clone(),
        ),
        SearchSlots::new(
            slots,
            application::catalog::CatalogLessonMenuLookup::new(
                application::in_memory::InMemoryLessonMenuRepository::default(),
            ),
            application::profile::ProfileCoachIntroductionLookup::new(
                InMemoryAccountRepository::default(),
            ),
            clock.clone(),
        ),
        CreateBooking::new(
            bookings.clone(),
            application::catalog::CatalogLessonMenuLookup::new(
                application::in_memory::InMemoryLessonMenuRepository::default(),
            ),
            analytics.clone(),
            clock.clone(),
        ),
        ConfirmBooking::new(
            bookings.clone(),
            EntitlementQuotaGate::new(entitlements.clone()),
            analytics.clone(),
            events.clone(),
            clock.clone(),
        ),
        CancelBooking::new(
            bookings.clone(),
            analytics.clone(),
            events.clone(),
            clock.clone(),
        ),
        GetSlotSupply::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            clock.clone(),
        ),
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
            application::catalog::CatalogLessonMenuLookup::new(
                application::in_memory::InMemoryLessonMenuRepository::default(),
            ),
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
        ),
        StartSubscription::new(
            entitlements.clone(),
            analytics.clone(),
            events.clone(),
            clock.clone(),
        ),
        GetEntitlement::new(entitlements, clock.clone()),
        StartLiveLessonSession::new(
            InMemoryLiveLessonSessionRepository::default(),
            SchedulingBookingLookup::new(bookings),
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
            videos.clone(),
            application::catalog::CatalogLessonMenuLookup::new(lesson_menus.clone()),
            FixedVideoStorage,
            FixedAiFeedbackGenerator,
            analytics.clone(),
            events.clone(),
            clock.clone(),
        ),
        GetVideoReview::new(videos.clone()),
        ReturnVideoReview::new(
            videos.clone(),
            EntitlementReviewGate,
            analytics.clone(),
            events.clone(),
            clock.clone(),
        ),
        application::async_lesson::ListCoachVideos::new(
            videos,
            application::profile::ProfileStudentDisplayNameLookup::new(accounts.clone()),
        ),
        RecordLessonCompletion::new(
            InMemoryLiveLessonSessionRepository::default(),
            InMemoryVideoRepository::default(),
            InMemoryLessonCompletionRepository::default(),
            RecordingAnalyticsSink::default(),
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
            events.clone(),
            clock.clone(),
        ),
        application::messaging::MarkMessageThreadRead::new(
            application::in_memory::InMemoryMessageThreadRepository::default(),
            clock.clone(),
        ),
        application::review::SubmitLessonReview::new(
            application::lesson::LessonCompletionLookup::new(
                application::in_memory::InMemoryLessonCompletionRepository::default(),
            ),
            application::in_memory::InMemoryLessonReviewRepository::default(),
            RecordingAnalyticsSink::default(),
            clock.clone(),
        ),
        application::review::GetCoachReviewSummary::new(
            application::in_memory::InMemoryLessonReviewRepository::default(),
        ),
        application::review::ListCoachReviews::new(
            application::in_memory::InMemoryLessonReviewRepository::default(),
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
        accounts,
        lesson_menus,
        router,
        clock,
        analytics,
        events,
    }
}

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn with_token(
    request: axum::http::request::Builder,
    token: Option<&str>,
) -> axum::http::request::Builder {
    match token {
        Some(token) => request.header(header::AUTHORIZATION, format!("Bearer {token}")),
        None => request,
    }
}

fn post_json(path: &str, token: Option<&str>, body: Value) -> Request<Body> {
    with_token(Request::post(path), token)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn tennis_video() -> Value {
    json!({
        "coach_id": account_id_uuid::format(account_id(COACH)),
        "sport": "tennis",
        "content_type": "video/mp4",
        "note": "サーブのトスが安定しない"
    })
}

fn review(comment: &str) -> Value {
    json!({ "comment": comment })
}

impl App {
    async fn submit(&self, token: Option<&str>, body: Value) -> (StatusCode, Value) {
        let (status, body) = send(&self.router, post_json("/videos", token, body)).await;
        support::Contract::load("student-app").assert_response("post", "/videos", status, &body);
        (status, body)
    }

    /// 生徒がテニスの動画を送る。動画 ID を返す。
    async fn submitted_video(&self) -> String {
        let (status, video) = self.submit(Some(STUDENT), tennis_video()).await;
        assert_eq!(status, StatusCode::CREATED, "{video}");
        video["video_id"].as_str().unwrap().to_string()
    }

    async fn get_review(&self, token: Option<&str>, video_id: &str) -> (StatusCode, Value) {
        let (status, body) = send(
            &self.router,
            with_token(Request::get(format!("/videos/{video_id}/review")), token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        support::Contract::load("student-app").assert_response(
            "get",
            "/videos/{video_id}/review",
            status,
            &body,
        );
        (status, body)
    }

    async fn return_review(
        &self,
        token: Option<&str>,
        video_id: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let (status, body) = send(
            &self.router,
            post_json(&format!("/videos/{video_id}/review"), token, body),
        )
        .await;
        support::Contract::load("admin-web").assert_response(
            "post",
            "/videos/{video_id}/review",
            status,
            &body,
        );
        (status, body)
    }

    /// `token` の生徒のアカウントを表示名 `display_name` で置く。
    async fn student_named(&self, token: &str, display_name: &str) {
        let account = Account::new(
            account_id(token),
            Email::new(format!("{}@example.com", account_id(token).as_u128())).unwrap(),
            DisplayName::new(display_name).unwrap(),
            Role::Student,
            [Sport::Tennis],
            submitted_at(),
        )
        .unwrap();
        self.accounts.insert(account).await.unwrap();
    }

    async fn list_coach_videos(&self, token: Option<&str>) -> (StatusCode, Value) {
        let (status, body) = send(
            &self.router,
            with_token(Request::get("/coaches/me/videos"), token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        support::Contract::load("admin-web").assert_response(
            "get",
            "/coaches/me/videos",
            status,
            &body,
        );
        (status, body)
    }

    /// 発火した `video.*` / `video_review.*` の行動イベント。
    fn video_events(&self) -> Vec<AnalyticsEvent> {
        self.analytics
            .events()
            .into_iter()
            .filter(|event| event.name().starts_with("video"))
            .collect()
    }
}

#[tokio::test]
async fn a_submitted_video_is_unreturned_until_the_coach_returns_its_review() {
    let app = app();

    // 送付: アップロード先を受け取り、video.submitted を発火する
    let (status, video) = app.submit(Some(STUDENT), tennis_video()).await;
    assert_eq!(status, StatusCode::CREATED, "{video}");
    let video_id = video["video_id"].as_str().unwrap().to_string();
    assert_eq!(
        video["student_id"],
        account_id_uuid::format(account_id(STUDENT))
    );
    assert_eq!(
        video["coach_id"],
        account_id_uuid::format(account_id(COACH))
    );
    assert_eq!(video["sport"], "tennis");
    assert_eq!(video["status"], "awaiting_upload");
    assert_eq!(
        video["upload_url"],
        format!("https://fixed-storage.invalid/videos/{video_id}")
    );
    assert_eq!(video["created_at"], "2026-10-26T10:00:00Z");
    let video = video_id_uuid::parse(&video_id).unwrap();
    assert_eq!(
        app.video_events(),
        vec![AnalyticsEvent::VideoSubmitted {
            student_id: account_id(STUDENT),
            coach_id: account_id(COACH),
            video_id: video,
            sport: Sport::Tennis,
            occurred_at: submitted_at(),
        }]
    );
    // 境界間イベント（messaging はこれでスレッドに添削依頼のメッセージを足す）
    let published = app.events.events();
    assert_eq!(published.len(), 1, "{published:?}");
    assert!(
        matches!(
            published[0],
            DomainEvent::VideoReviewRequested { occurred_at, student_id, coach_id, video_id, .. }
                if occurred_at == submitted_at()
                    && student_id == account_id(STUDENT)
                    && coach_id == account_id(COACH)
                    && video_id == video
        ),
        "{published:?}"
    );

    // 返却前の取得は未返却（生徒からもコーチからも）
    for token in [STUDENT, COACH] {
        let (status, body) = app.get_review(Some(token), &video_id).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["status"], "in_progress");
        assert_eq!(body["video_id"], video_id.as_str());
        assert_eq!(body["review_id"], Value::Null);
        assert_eq!(body["comment"], Value::Null);
        assert_eq!(body["returned_at"], Value::Null);
        // AI の一次フィードバックは送付の直後に付いている（添削とは別の項目）
        assert_eq!(
            body["ai_feedback"],
            fixed_ai_feedback("2026-10-26T10:00:00Z")
        );
    }

    // 30 時間後にコーチが返却する
    let returned_at = submitted_at() + Duration::from_secs(30 * 3600);
    app.clock.set(returned_at);
    let (status, returned) = app
        .return_review(Some(COACH), &video_id, review("トスを体の前に上げる"))
        .await;
    assert_eq!(status, StatusCode::OK, "{returned}");
    assert_eq!(returned["status"], "returned");
    assert_eq!(returned["comment"], "トスを体の前に上げる");
    assert_eq!(returned["returned_at"], "2026-10-27T16:00:00Z");
    assert_eq!(
        returned["ai_feedback"],
        fixed_ai_feedback("2026-10-26T10:00:00Z"),
        "コーチの返却で上書きされない"
    );
    let review_id = returned["review_id"].as_str().unwrap();

    // 生徒が読む添削は、コーチが返したものと同じ
    let (status, body) = app.get_review(Some(STUDENT), &video_id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, returned);

    // video_review.returned は送付の時刻も持つ（返却のリードタイム = occurred_at - submitted_at）
    let events = app.video_events();
    assert_eq!(events.len(), 2, "{events:?}");
    assert_eq!(
        events[1],
        AnalyticsEvent::VideoReviewReturned {
            student_id: account_id(STUDENT),
            coach_id: account_id(COACH),
            video_id: video,
            review_id: video_review_id_uuid::parse(review_id).unwrap(),
            submitted_at: submitted_at(),
            occurred_at: returned_at,
        }
    );
    assert_eq!(events[1].name(), "video_review.returned");
}

/// [`FixedAiFeedbackGenerator`] が `created_at` に作った一次フィードバックの契約の形。
fn fixed_ai_feedback(created_at: &str) -> Value {
    json!({
        "comment": FixedAiFeedbackGenerator::COMMENT,
        "created_at": created_at,
    })
}

#[tokio::test]
async fn monthly_students_can_submit_videos_without_limit() {
    let app = app();
    for _ in 0..3 {
        app.submitted_video().await;
    }
    assert_eq!(app.video_events().len(), 3);
}

#[tokio::test]
async fn a_review_is_returned_only_once() {
    let app = app();
    let video_id = app.submitted_video().await;
    let (status, _) = app
        .return_review(Some(COACH), &video_id, review("1 回目"))
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = app
        .return_review(Some(COACH), &video_id, review("2 回目"))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "invalid_video_state");
    let (_, body) = app.get_review(Some(STUDENT), &video_id).await;
    assert_eq!(body["comment"], "1 回目", "最初の返却のまま");
    assert_eq!(app.video_events().len(), 2, "送付と 1 回目の返却だけ");
}

#[tokio::test]
async fn golf_reviews_cannot_start_without_a_confirmed_one_shot_payment() {
    let app = app();
    let golf_video = video_id_uuid::format(GOLF_VIDEO);

    let (status, body) = app
        .return_review(Some(COACH), &golf_video, review("グリップが強い"))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "review_payment_not_confirmed");
    let (_, body) = app.get_review(Some(STUDENT), &golf_video).await;
    assert_eq!(body["status"], "in_progress");
    assert!(app.video_events().is_empty());
}

#[tokio::test]
async fn invalid_submissions_are_unprocessable() {
    let app = app();
    let with = |key: &str, value: Value| {
        let mut body = tennis_video();
        body[key] = value;
        body
    };
    for body in [
        // ゴルフは golf-app の範囲
        with("sport", json!("golf")),
        with("sport", json!("cricket")),
        with("coach_id", json!("not-a-uuid")),
        with("content_type", json!("video/webm")),
        with("note", json!("あ".repeat(1001))),
        json!({ "sport": "tennis" }),
    ] {
        let (status, error) = app.submit(Some(STUDENT), body.clone()).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body} {error}");
        assert_eq!(error["code"], "validation_failed");
    }
    // note は任意で、1000 文字までは受け付ける
    let mut body = tennis_video();
    body.as_object_mut().unwrap().remove("note");
    assert_eq!(app.submit(Some(STUDENT), body).await.0, StatusCode::CREATED);
    let body = with("note", json!("あ".repeat(1000)));
    assert_eq!(app.submit(Some(STUDENT), body).await.0, StatusCode::CREATED);
}

#[tokio::test]
async fn invalid_comments_are_unprocessable() {
    let app = app();
    let video_id = app.submitted_video().await;
    for body in [
        review(""),
        review(" \u{3000}"),
        review(&"あ".repeat(4001)),
        json!({}),
    ] {
        let (status, error) = app
            .return_review(Some(COACH), &video_id, body.clone())
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body} {error}");
        assert_eq!(error["code"], "validation_failed");
    }
    let (_, body) = app.get_review(Some(STUDENT), &video_id).await;
    assert_eq!(body["status"], "in_progress");
}

#[tokio::test]
async fn only_students_submit_and_only_coaches_return() {
    let app = app();
    for token in [COACH, HEADQUARTERS] {
        let (status, body) = app.submit(Some(token), tennis_video()).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        assert_eq!(body["code"], "forbidden");
    }
    let video_id = app.submitted_video().await;
    for token in [STUDENT, HEADQUARTERS] {
        let (status, body) = app
            .return_review(Some(token), &video_id, review("コメント"))
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        assert_eq!(body["code"], "forbidden");
    }

    assert_eq!(
        app.submit(None, tennis_video()).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.get_review(None, &video_id).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.return_review(None, &video_id, review("コメント"))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn videos_of_others_are_not_found() {
    let app = app();
    let video_id = app.submitted_video().await;

    // 当事者でない生徒・コーチには、動画があることも知らせない
    for token in [OTHER_STUDENT, OTHER_COACH, HEADQUARTERS] {
        let (status, body) = app.get_review(Some(token), &video_id).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{token} {body}");
        assert_eq!(body["code"], "not_found");
    }
    let (status, body) = app
        .return_review(Some(OTHER_COACH), &video_id, review("コメント"))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");

    // ない動画・UUID でない ID も 404
    let missing = video_id_uuid::format(VideoId::from_u128(404));
    for id in [missing.as_str(), "not-a-uuid"] {
        assert_eq!(
            app.get_review(Some(STUDENT), id).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            app.return_review(Some(COACH), id, review("コメント"))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(app.video_events().len(), 1, "送付だけ");
}

#[tokio::test]
async fn coaches_list_their_own_videos_awaiting_review_first_then_latest_returned() {
    let app = app();
    app.student_named(STUDENT, "あおき").await;
    let golf = video_id_uuid::format(GOLF_VIDEO);

    // 1 時間後に生徒 1、2 時間後に生徒 2（アカウントがなく表示名を引けない）が送り、3 時間後に生徒 1 の動画を返却する
    let hour = |h: u64| submitted_at() + Duration::from_secs(h * 3600);
    app.clock.set(hour(1));
    let first = app.submitted_video().await;
    app.clock.set(hour(2));
    let (status, second) = app.submit(Some(OTHER_STUDENT), tennis_video()).await;
    assert_eq!(status, StatusCode::CREATED, "{second}");
    let second = second["video_id"].as_str().unwrap().to_string();
    app.clock.set(hour(3));
    let (status, body) = app
        .return_review(Some(COACH), &first, review("トスを体の前に上げる"))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = app.list_coach_videos(Some(COACH)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // 添削待ちが送られた時刻の古い順で先（ゴルフ → 生徒 2）、返却済みが後。別のコーチに送られた動画は出ない
    assert_eq!(
        body,
        json!({
            "videos": [
                {
                    "video_id": golf,
                    "student_display_name": "あおき",
                    "submitted_at": "2026-10-26T10:00:00Z",
                    "review_status": "in_progress",
                    // 一次フィードバックなしで保存された動画（AI が作れなかった）は null
                    "ai_feedback": null,
                    // メニューを選ばずに送られた動画は null
                    "lesson_menu": null
                },
                {
                    "video_id": second,
                    "submitted_at": "2026-10-26T12:00:00Z",
                    "review_status": "in_progress",
                    "ai_feedback": fixed_ai_feedback("2026-10-26T12:00:00Z"),
                    "lesson_menu": null
                },
                {
                    "video_id": first,
                    "student_display_name": "あおき",
                    "submitted_at": "2026-10-26T11:00:00Z",
                    "review_status": "returned",
                    "returned_at": "2026-10-26T13:00:00Z",
                    // 返却の後も、送付の直後に作った一次フィードバックのまま
                    "ai_feedback": fixed_ai_feedback("2026-10-26T11:00:00Z"),
                    "lesson_menu": null
                }
            ]
        })
    );

    // 別のコーチには、そのコーチに送られた動画だけ
    let (status, body) = app.list_coach_videos(Some(OTHER_COACH)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ids: Vec<_> = body["videos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|video| video["video_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [video_id_uuid::format(OTHER_COACH_VIDEO)]);

    // 動画が届いていないコーチは空の配列
    let (status, body) = app.list_coach_videos(Some("coach:3")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "videos": [] }));
}

#[tokio::test]
async fn only_coaches_list_their_videos() {
    let app = app();
    for token in [STUDENT, HEADQUARTERS] {
        let (status, body) = app.list_coach_videos(Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token} {body}");
        assert_eq!(body["code"], "forbidden");
    }
    assert_eq!(
        app.list_coach_videos(None).await.0,
        StatusCode::UNAUTHORIZED
    );
}

fn tennis_video_with_menu(lesson_menu_id: &str) -> Value {
    let mut body = tennis_video();
    body["lesson_menu_id"] = json!(lesson_menu_id);
    body
}

#[tokio::test]
async fn the_chosen_lesson_menu_is_copied_to_the_video_as_of_submission() {
    let app = app();

    let menu_id = lesson_menu_id_uuid::format(MENU);
    let (status, video) = app
        .submit(Some(STUDENT), tennis_video_with_menu(&menu_id))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{video}");
    let expected = json!({ "lesson_menu_id": menu_id, "name": "フォーム添削" });
    assert_eq!(video["lesson_menu"], expected);
    let video_id = video["video_id"].as_str().unwrap().to_string();

    // 送った後にコーチがメニューの名前を直し、非公開にしても、動画の表示は送った時点のまま
    app.lesson_menus
        .update(menu(
            MENU,
            COACH,
            Sport::Tennis,
            LessonFormat::Async,
            "スイング添削",
            false,
        ))
        .await
        .unwrap();

    for token in [STUDENT, COACH] {
        let (status, body) = app.get_review(Some(token), &video_id).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["lesson_menu"], expected);
    }
    let (status, listed) = app.list_coach_videos(Some(COACH)).await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    let listed = listed["videos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["video_id"] == video_id.as_str())
        .unwrap()
        .clone();
    assert_eq!(listed["lesson_menu"], expected);

    let (status, returned) = app
        .return_review(Some(COACH), &video_id, review("トスを体の前に上げる"))
        .await;
    assert_eq!(status, StatusCode::OK, "{returned}");
    assert_eq!(returned["lesson_menu"], expected, "返却で変わらない");
}

#[tokio::test]
async fn a_video_sent_without_a_lesson_menu_has_a_null_lesson_menu() {
    let app = app();

    let (status, video) = app.submit(Some(STUDENT), tennis_video()).await;
    assert_eq!(status, StatusCode::CREATED, "{video}");
    assert_eq!(video["lesson_menu"], Value::Null);
    let video_id = video["video_id"].as_str().unwrap().to_string();

    let (_, body) = app.get_review(Some(STUDENT), &video_id).await;
    assert_eq!(body["lesson_menu"], Value::Null);
    let (_, listed) = app.list_coach_videos(Some(COACH)).await;
    for video in listed["videos"].as_array().unwrap() {
        assert_eq!(video["lesson_menu"], Value::Null, "{video}");
    }
    let (_, returned) = app
        .return_review(Some(COACH), &video_id, review("トスを体の前に上げる"))
        .await;
    assert_eq!(returned["lesson_menu"], Value::Null);
}

#[tokio::test]
async fn a_lesson_menu_that_cannot_be_chosen_is_rejected_with_422_and_nothing_is_accepted() {
    for (lesson_menu_id, message) in [
        (
            lesson_menu_id_uuid::format(OTHER_COACH_MENU),
            "動画で選べるのは送り先のコーチのレッスンメニューだけ",
        ),
        (
            lesson_menu_id_uuid::format(UNPUBLISHED_MENU),
            "公開中でないレッスンメニューは動画で選べない",
        ),
        (
            lesson_menu_id_uuid::format(LIVE_MENU),
            "同期レッスンのメニューは動画で選べない（非同期レッスンのメニューだけ）",
        ),
        (
            lesson_menu_id_uuid::format(SOCCER_MENU),
            "レッスンメニューのスポーツ（soccer）が動画のスポーツ（tennis）と違う",
        ),
        (
            lesson_menu_id_uuid::format(LessonMenuId::from_u128(99)),
            "指定したレッスンメニューがない",
        ),
        ("menu-50".to_string(), "lesson_menu_id が UUID でない"),
    ] {
        let app = app();

        let (status, body) = app
            .submit(Some(STUDENT), tennis_video_with_menu(&lesson_menu_id))
            .await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(body["code"], "validation_failed");
        assert_eq!(body["message"], message);
        assert!(
            app.video_events().is_empty(),
            "video.submitted は発火しない"
        );
        assert!(app.events.events().is_empty(), "添削依頼は流さない");
        let (_, listed) = app.list_coach_videos(Some(COACH)).await;
        assert_eq!(
            listed["videos"].as_array().unwrap().len(),
            1,
            "先に置いたゴルフの動画だけ（受け付けていない）"
        );
    }
}
