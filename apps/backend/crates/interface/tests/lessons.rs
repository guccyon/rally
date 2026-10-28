//! `/lessons/completions` の統合テスト。student-app の `POST /lessons/completions`（`recordLessonCompletion`）の
//! レスポンスの形を OpenAPI のスキーマで検査し、通話（終了した通話セッション）と動画（添削を返した動画）の両方から
//! 完了を記録できること・同じ生徒の 2 回目で `is_first_lesson` が false になること・行動イベント `lesson.completed`
//! と境界間イベント `LessonCompleted`・409 / 404 / 422 / 401 を確かめる。

use std::time::{Duration, SystemTime};

use application::{
    analytics::AnalyticsEvent,
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo, video_id_uuid},
    domain_event::DomainEvent,
    entitlement::{EntitlementQuotaGate, EntitlementReviewGate, GetEntitlement, StartSubscription},
    identity::{
        AccessToken, InvalidToken, RegisterAccount, TokenClaims, TokenIssuer, TokenVerifier,
        account_id_uuid,
    },
    in_memory::{
        FixedClock, FixedVideoCallProvider, FixedVideoStorage, InMemoryAccountRepository,
        InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemoryLessonCompletionRepository, InMemoryLiveLessonSessionRepository,
        InMemorySlotRepository, InMemoryVideoRepository, RecordingAnalyticsSink,
        RecordingDomainEventPublisher,
    },
    lesson::{RecordLessonCompletion, lesson_id_uuid},
    live_lesson::{EndLiveLessonSession, StartLiveLessonSession, session_id_uuid},
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
    async_lesson::{ReviewComment, Video, VideoId, VideoReviewId},
    identity::{AccountId, Role, Sport},
    lesson::LessonType,
    live_lesson::{ConnectionQuality, LiveLessonSession, SessionId},
    scheduling::{Booking, BookingId, Slot, SlotId, SlotStatus},
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
const HEADQUARTERS: &str = "headquarters:5";

/// 生徒 1 とコーチ 9 の終了した通話セッション
const ENDED_SESSION: SessionId = SessionId::from_u128(30);
/// 生徒 1 とコーチ 9 の、まだ終了していない通話セッション
const OPEN_SESSION: SessionId = SessionId::from_u128(31);
/// 生徒 1 がコーチ 9 に送り、添削が返された動画
const REVIEWED_VIDEO: VideoId = VideoId::from_u128(40);
/// 生徒 1 がコーチ 9 に送り、添削がまだ返されていない動画
const UNREVIEWED_VIDEO: VideoId = VideoId::from_u128(41);

fn account_id(token: &str) -> AccountId {
    AccountId::from_u128(token.split_once(':').unwrap().1.parse().unwrap())
}

/// 2026-11-01T10:00:00Z（通話の開始・動画の送付）
fn nov1_10() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200 + 10 * 3600)
}

/// 完了を記録する時刻（2026-11-01T12:00:00Z）
fn now() -> SystemTime {
    nov1_10() + Duration::from_secs(2 * 3600)
}

fn session(id: SessionId, booking: u128, ended: bool) -> LiveLessonSession {
    let mut slot = Slot::restore(
        SlotId::from_u128(booking),
        account_id(COACH),
        Sport::Tennis,
        nov1_10(),
        nov1_10() + Duration::from_secs(3600),
        SlotStatus::Open,
    )
    .unwrap();
    let mut booking = Booking::reserve(
        BookingId::from_u128(booking),
        account_id(STUDENT),
        false,
        &slot,
        &[],
        nov1_10(),
    )
    .unwrap();
    booking
        .confirm(account_id(STUDENT), &mut slot, nov1_10())
        .unwrap();
    let mut session =
        LiveLessonSession::start(id, account_id(STUDENT), &booking, None, nov1_10()).unwrap();
    if ended {
        session
            .end(
                account_id(COACH),
                nov1_10() + Duration::from_secs(3600),
                ConnectionQuality::new(0.0, 10, 50, 0).unwrap(),
            )
            .unwrap();
    }
    session
}

fn video(id: VideoId, reviewed: bool) -> Video {
    let mut video = Video::submit(
        id,
        account_id(STUDENT),
        account_id(COACH),
        Sport::Tennis,
        nov1_10(),
    );
    if reviewed {
        video
            .return_review(
                VideoReviewId::from_u128(id.as_u128()),
                account_id(COACH),
                ReviewComment::new("テイクバックが早い").unwrap(),
                true,
                nov1_10() + Duration::from_secs(3600),
            )
            .unwrap();
    }
    video
}

struct App {
    router: Router,
    analytics: RecordingAnalyticsSink,
    events: RecordingDomainEventPublisher,
}

/// 生徒 1 とコーチ 9 の通話セッション 30（終了）・31（通話中）と、動画 40（添削を返した）・41（未返却）を置く。
/// 現在時刻は 2026-11-01 12:00。
fn app() -> App {
    let analytics = RecordingAnalyticsSink::default();
    let events = RecordingDomainEventPublisher::default();
    let clock = FixedClock(now());
    let slots = InMemorySlotRepository::default();
    let bookings = InMemoryBookingRepository::new(slots.clone());
    let entitlements = InMemoryEntitlementRepository::default();
    let sessions = InMemoryLiveLessonSessionRepository::with_sessions([
        session(ENDED_SESSION, 10, true),
        session(OPEN_SESSION, 11, false),
    ]);
    let videos = InMemoryVideoRepository::with_videos([
        video(REVIEWED_VIDEO, true),
        video(UNREVIEWED_VIDEO, false),
    ]);
    let router = interface::router_with(
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            analytics.clone(),
            clock,
        ),
        application::identity::ListCoaches::new(InMemoryAccountRepository::default()),
        application::identity::GetAccount::new(InMemoryAccountRepository::default()),
        PublishSlot::new(slots.clone(), analytics.clone(), clock),
        SearchSlots::new(slots, clock),
        CreateBooking::new(bookings.clone(), analytics.clone(), clock),
        ConfirmBooking::new(
            bookings.clone(),
            EntitlementQuotaGate::new(entitlements.clone()),
            analytics.clone(),
            events.clone(),
            clock,
        ),
        CancelBooking::new(bookings.clone(), analytics.clone(), events.clone(), clock),
        GetSlotSupply::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            clock,
        ),
        StartSubscription::new(
            entitlements.clone(),
            analytics.clone(),
            events.clone(),
            clock,
        ),
        GetEntitlement::new(entitlements, clock),
        StartLiveLessonSession::new(
            sessions.clone(),
            SchedulingBookingLookup::new(bookings),
            FixedVideoCallProvider::default(),
            analytics.clone(),
            clock,
        ),
        EndLiveLessonSession::new(
            sessions.clone(),
            FixedVideoCallProvider::default(),
            analytics.clone(),
            clock,
        ),
        SubmitVideo::new(videos.clone(), FixedVideoStorage, analytics.clone(), clock),
        GetVideoReview::new(videos.clone()),
        ReturnVideoReview::new(
            videos.clone(),
            EntitlementReviewGate,
            analytics.clone(),
            events.clone(),
            clock,
        ),
        RecordLessonCompletion::new(
            sessions,
            videos,
            InMemoryLessonCompletionRepository::default(),
            analytics.clone(),
            events.clone(),
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
            events.clone(),
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
        ),
        FakeTokens,
    );
    App {
        router,
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

fn post_json(path: &str, token: Option<&str>, body: Value) -> Request<Body> {
    let mut request = Request::post(path).header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    request.body(Body::from(body.to_string())).unwrap()
}

fn live(session_id: SessionId) -> Value {
    json!({ "lesson_type": "live_lesson", "session_id": session_id_uuid::format(session_id) })
}

fn asynchronous(video_id: VideoId) -> Value {
    json!({ "lesson_type": "async_lesson", "video_id": video_id_uuid::format(video_id) })
}

impl App {
    /// `token` のアカウントがレッスンの完了を記録する。レスポンスの形を契約で確かめる。
    async fn complete(&self, token: &str, body: Value) -> (StatusCode, Value) {
        let (status, body) = send(
            &self.router,
            post_json("/lessons/completions", Some(token), body),
        )
        .await;
        support::Contract::load("student-app").assert_response(
            "post",
            "/lessons/completions",
            status,
            &body,
        );
        (status, body)
    }

    /// 発火した `lesson.completed` の `(lesson_type, is_first_lesson)`（発火した順）。
    fn lesson_completed(&self) -> Vec<(LessonType, bool)> {
        self.analytics
            .events()
            .into_iter()
            .filter_map(|event| match event {
                AnalyticsEvent::LessonCompleted {
                    lesson_type,
                    is_first_lesson,
                    ..
                } => Some((lesson_type, is_first_lesson)),
                _ => None,
            })
            .collect()
    }
}

#[tokio::test]
async fn a_live_lesson_is_completed_from_an_ended_session() {
    let app = app();

    let (status, lesson) = app.complete(COACH, live(ENDED_SESSION)).await;

    assert_eq!(status, StatusCode::CREATED, "{lesson}");
    assert_eq!(lesson["lesson_type"], "live_lesson");
    assert_eq!(lesson["session_id"], session_id_uuid::format(ENDED_SESSION));
    assert_eq!(lesson["video_id"], Value::Null);
    assert_eq!(
        lesson["student_id"],
        account_id_uuid::format(account_id(STUDENT))
    );
    assert_eq!(
        lesson["coach_id"],
        account_id_uuid::format(account_id(COACH))
    );
    assert_eq!(lesson["is_first_lesson"], true);
    assert_eq!(lesson["completed_at"], "2026-11-01T12:00:00Z");

    let lesson_id = lesson_id_uuid::parse(lesson["lesson_id"].as_str().unwrap()).unwrap();
    assert_eq!(
        app.analytics.events(),
        vec![AnalyticsEvent::LessonCompleted {
            student_id: account_id(STUDENT),
            coach_id: account_id(COACH),
            lesson_id,
            lesson_type: LessonType::Live,
            is_first_lesson: true,
            occurred_at: now(),
        }]
    );
    let events = app.events.events();
    assert!(
        matches!(
            events[..],
            [DomainEvent::LessonCompleted { lesson_id: id, student_id, coach_id, .. }]
                if id == lesson_id && student_id == account_id(STUDENT) && coach_id == account_id(COACH)
        ),
        "{events:?}"
    );
}

#[tokio::test]
async fn an_async_lesson_is_completed_from_a_reviewed_video() {
    let app = app();

    let (status, lesson) = app.complete(STUDENT, asynchronous(REVIEWED_VIDEO)).await;

    assert_eq!(status, StatusCode::CREATED, "{lesson}");
    assert_eq!(lesson["lesson_type"], "async_lesson");
    assert_eq!(lesson["video_id"], video_id_uuid::format(REVIEWED_VIDEO));
    assert_eq!(lesson["session_id"], Value::Null);
    assert_eq!(lesson["is_first_lesson"], true);
    assert_eq!(app.lesson_completed(), [(LessonType::Async, true)]);
    assert_eq!(app.events.events().len(), 1);
}

#[tokio::test]
async fn the_second_lesson_of_the_same_student_is_not_the_first() {
    let app = app();

    let (_, first) = app.complete(STUDENT, live(ENDED_SESSION)).await;
    let (status, second) = app.complete(COACH, asynchronous(REVIEWED_VIDEO)).await;

    assert_eq!(status, StatusCode::CREATED, "{second}");
    assert_eq!(first["is_first_lesson"], true);
    assert_eq!(
        second["is_first_lesson"], false,
        "通話の後の動画添削は 2 回目"
    );
    assert_ne!(first["lesson_id"], second["lesson_id"]);
    assert_eq!(
        app.lesson_completed(),
        [(LessonType::Live, true), (LessonType::Async, false)]
    );
    assert_eq!(app.events.events().len(), 2);
}

#[tokio::test]
async fn unfinished_lessons_and_repeats_conflict() {
    let app = app();

    for (body, code) in [
        (live(OPEN_SESSION), "invalid_session_state"),
        (asynchronous(UNREVIEWED_VIDEO), "invalid_video_state"),
    ] {
        let (status, error) = app.complete(STUDENT, body).await;
        assert_eq!(status, StatusCode::CONFLICT, "{error}");
        assert_eq!(error["code"], code);
    }

    app.complete(STUDENT, live(ENDED_SESSION)).await;
    app.complete(STUDENT, asynchronous(REVIEWED_VIDEO)).await;
    for (token, body) in [
        (COACH, live(ENDED_SESSION)),
        (STUDENT, asynchronous(REVIEWED_VIDEO)),
    ] {
        let (status, error) = app.complete(token, body).await;
        assert_eq!(status, StatusCode::CONFLICT, "{error}");
        assert_eq!(error["code"], "lesson_already_completed");
    }
    assert_eq!(app.lesson_completed().len(), 2, "拒否では発火しない");
    assert_eq!(app.events.events().len(), 2);
}

#[tokio::test]
async fn non_parties_and_unknown_ids_are_not_found() {
    let app = app();

    for (token, body) in [
        (OTHER_STUDENT, live(ENDED_SESSION)),
        (HEADQUARTERS, asynchronous(REVIEWED_VIDEO)),
        (STUDENT, live(SessionId::from_u128(99))),
        (STUDENT, asynchronous(VideoId::from_u128(99))),
    ] {
        let (status, error) = app.complete(token, body.clone()).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{token} {body}: {error}");
        assert_eq!(error["code"], "not_found");
    }
    assert!(app.analytics.events().is_empty());
    assert!(app.events.events().is_empty());
}

#[tokio::test]
async fn malformed_requests_and_missing_tokens_are_rejected() {
    let app = app();
    let session_id = session_id_uuid::format(ENDED_SESSION);
    let video_id = video_id_uuid::format(REVIEWED_VIDEO);

    for body in [
        json!({ "lesson_type": "live_lesson", "session_id": "not-a-uuid" }),
        json!({ "lesson_type": "async_lesson", "video_id": "not-a-uuid" }),
        // 判別子に合わない ID・両方の ID・ID なし
        json!({ "lesson_type": "live_lesson", "video_id": video_id }),
        json!({ "lesson_type": "async_lesson", "session_id": session_id }),
        json!({ "lesson_type": "live_lesson", "session_id": session_id, "video_id": video_id }),
        json!({ "lesson_type": "live_lesson" }),
        json!({ "lesson_type": "group_lesson", "session_id": session_id }),
        json!({ "session_id": session_id }),
        // 契約は additionalProperties: false
        json!({ "lesson_type": "live_lesson", "session_id": session_id, "note": "x" }),
    ] {
        let (status, error) = app.complete(STUDENT, body.clone()).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}: {error}");
        assert_eq!(error["code"], "validation_failed");
    }

    let (status, error) = send(
        &app.router,
        post_json("/lessons/completions", None, live(ENDED_SESSION)),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    support::Contract::load("student-app").assert_response(
        "post",
        "/lessons/completions",
        status,
        &error,
    );
    assert!(app.analytics.events().is_empty());
}
