//! `/live-lesson-sessions` の統合テスト。student-app の `POST /live-lesson-sessions`（`startLiveLessonSession`）と
//! `POST /live-lesson-sessions/{session_id}/end`（`endLiveLessonSession`）のレスポンスの形を OpenAPI のスキーマで
//! 検査し、確定前の予約では開始できないこと・同じ予約のセッションは 1 つ・終了で接続品質の 4 属性が
//! 行動イベント `call.ended` に記録されること・参加者以外の 404 を確かめる。

use std::time::{Duration, SystemTime};

use application::{
    analytics::AnalyticsEvent,
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo},
    entitlement::{EntitlementQuotaGate, EntitlementReviewGate, GetEntitlement, StartSubscription},
    identity::{
        AccessToken, InvalidToken, RegisterAccount, TokenClaims, TokenIssuer, TokenVerifier,
        account_id_uuid,
    },
    in_memory::{
        FixedVideoCallProvider, FixedVideoStorage, InMemoryAccountRepository,
        InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemoryLessonCompletionRepository, InMemoryLiveLessonSessionRepository,
        InMemorySlotRepository, InMemoryVideoRepository, ManualClock, RecordingAnalyticsSink,
        RecordingDomainEventPublisher,
    },
    lesson::RecordLessonCompletion,
    live_lesson::{EndLiveLessonSession, StartLiveLessonSession, session_id_uuid},
    scheduling::{
        CancelBooking, ConfirmBooking, CreateBooking, GetSlotSupply, PublishSlot,
        SchedulingBookingLookup, SearchSlots, booking_id_uuid, slot_id_uuid,
    },
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    identity::{AccountId, Role, Sport},
    live_lesson::{ConnectionQuality, SessionId},
    scheduling::{BookingId, Slot, SlotId, SlotStatus},
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

fn account_id(token: &str) -> AccountId {
    AccountId::from_u128(token.split_once(':').unwrap().1.parse().unwrap())
}

/// 2026-11-01T10:00:00Z（スロットの開始。通話を始める時刻）
fn nov1_10() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200 + 10 * 3600)
}

/// 通話基盤（偽物）が終了時に返す接続品質
fn quality() -> ConnectionQuality {
    ConnectionQuality::new(3.25, 48, 210, 1).unwrap()
}

struct App {
    router: Router,
    clock: ManualClock,
    analytics: RecordingAnalyticsSink,
}

/// コーチのスロット 10（2026-11-01 10:00 から 1 時間、公開中）を置く。現在時刻は 2026-10-26。
fn app() -> App {
    let analytics = RecordingAnalyticsSink::default();
    let events = RecordingDomainEventPublisher::default();
    let clock = ManualClock::new(nov1_10() - Duration::from_secs(6 * 86_400));
    let slots = InMemorySlotRepository::with_slots([Slot::restore(
        SlotId::from_u128(10),
        account_id(COACH),
        Sport::Tennis,
        nov1_10(),
        nov1_10() + Duration::from_secs(3600),
        SlotStatus::Open,
    )
    .unwrap()]);
    let bookings = InMemoryBookingRepository::new(slots.clone());
    let entitlements = InMemoryEntitlementRepository::default();
    let sessions = InMemoryLiveLessonSessionRepository::default();
    let video = FixedVideoCallProvider::new(quality());
    let router = interface::router_with(
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            analytics.clone(),
            clock.clone(),
        ),
        PublishSlot::new(slots.clone(), analytics.clone(), clock.clone()),
        SearchSlots::new(slots, clock.clone()),
        CreateBooking::new(bookings.clone(), analytics.clone(), clock.clone()),
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
        StartSubscription::new(
            entitlements.clone(),
            analytics.clone(),
            events,
            clock.clone(),
        ),
        GetEntitlement::new(entitlements, clock.clone()),
        StartLiveLessonSession::new(
            sessions.clone(),
            SchedulingBookingLookup::new(bookings),
            video.clone(),
            analytics.clone(),
            clock.clone(),
        ),
        EndLiveLessonSession::new(sessions, video, analytics.clone(), clock.clone()),
        SubmitVideo::new(
            InMemoryVideoRepository::default(),
            FixedVideoStorage,
            RecordingAnalyticsSink::default(),
            clock.clone(),
        ),
        GetVideoReview::new(InMemoryVideoRepository::default()),
        ReturnVideoReview::new(
            InMemoryVideoRepository::default(),
            EntitlementReviewGate,
            RecordingAnalyticsSink::default(),
            clock.clone(),
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
            clock.clone(),
        ),
        application::messaging::MarkMessageThreadRead::new(
            application::in_memory::InMemoryMessageThreadRepository::default(),
            clock.clone(),
        ),
        FakeTokens,
    );
    App {
        router,
        clock,
        analytics,
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

impl App {
    /// 生徒がスロット 10 を予約する（確定前）。予約 ID を返す。
    async fn book(&self) -> String {
        let (status, booking) = send(
            &self.router,
            post_json(
                "/bookings",
                Some(STUDENT),
                json!({ "slot_id": slot_id_uuid::format(SlotId::from_u128(10)), "is_trial": true }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{booking}");
        booking["booking_id"].as_str().unwrap().to_string()
    }

    /// 生徒がスロット 10 を予約して確定する。予約 ID を返す。
    async fn confirmed_booking(&self) -> String {
        let booking_id = self.book().await;
        let (status, body) = send(
            &self.router,
            post_json(
                &format!("/bookings/{booking_id}/confirm"),
                Some(STUDENT),
                json!({}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        booking_id
    }

    async fn start(&self, token: &str, booking_id: &str) -> (StatusCode, Value) {
        let (status, body) = send(
            &self.router,
            post_json(
                "/live-lesson-sessions",
                Some(token),
                json!({ "booking_id": booking_id }),
            ),
        )
        .await;
        support::Contract::load("student-app").assert_response(
            "post",
            "/live-lesson-sessions",
            status,
            &body,
        );
        (status, body)
    }

    async fn end(&self, token: &str, session_id: &str) -> (StatusCode, Value) {
        let (status, body) = send(
            &self.router,
            post_json(
                &format!("/live-lesson-sessions/{session_id}/end"),
                Some(token),
                json!({}),
            ),
        )
        .await;
        support::Contract::load("student-app").assert_response(
            "post",
            "/live-lesson-sessions/{session_id}/end",
            status,
            &body,
        );
        (status, body)
    }

    /// 発火した `call.*` の行動イベント。
    fn call_events(&self) -> Vec<AnalyticsEvent> {
        self.analytics
            .events()
            .into_iter()
            .filter(|e| e.name().starts_with("call."))
            .collect()
    }
}

#[tokio::test]
async fn a_booking_that_is_not_confirmed_cannot_be_started() {
    let app = app();
    let booking_id = app.book().await;
    app.clock.set(nov1_10());

    let (status, body) = app.start(STUDENT, &booking_id).await;

    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "invalid_booking_state");
    assert!(app.call_events().is_empty(), "call.started を発火しない");
}

#[tokio::test]
async fn starting_and_ending_records_the_session_and_the_four_metrics() {
    let app = app();
    let booking_id = app.confirmed_booking().await;
    app.clock.set(nov1_10());

    // 開始
    let (status, started) = app.start(STUDENT, &booking_id).await;
    assert_eq!(status, StatusCode::CREATED, "{started}");
    assert_eq!(started["booking_id"], booking_id.as_str());
    assert_eq!(
        started["student_id"],
        account_id_uuid::format(account_id(STUDENT))
    );
    assert_eq!(
        started["coach_id"],
        account_id_uuid::format(account_id(COACH))
    );
    assert_eq!(started["status"], "in_progress");
    assert_eq!(started["started_at"], "2026-11-01T10:00:00Z");
    assert_eq!(started["ended_at"], Value::Null);
    assert_eq!(started["duration_sec"], Value::Null);
    assert!(
        started["room_token"]
            .as_str()
            .is_some_and(|t| !t.is_empty())
    );
    let session_id = started["session_id"].as_str().unwrap().to_string();
    let session = session_id_uuid::parse(&session_id).unwrap();
    let booking = booking_id_uuid::parse(&booking_id).unwrap();
    assert_eq!(
        app.call_events(),
        vec![AnalyticsEvent::CallStarted {
            student_id: account_id(STUDENT),
            coach_id: account_id(COACH),
            booking_id: booking,
            session_id: session,
            occurred_at: nov1_10(),
        }]
    );

    // 同じ予約の 2 回目の開始は、コーチからでも 409
    let (status, body) = app.start(COACH, &booking_id).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "invalid_session_state");

    // 50 分 15 秒後にコーチが終了する
    let ended_at = nov1_10() + Duration::from_secs(50 * 60 + 15);
    app.clock.set(ended_at);
    let (status, ended) = app.end(COACH, &session_id).await;
    assert_eq!(status, StatusCode::OK, "{ended}");
    assert_eq!(ended["session_id"], session_id.as_str());
    assert_eq!(ended["status"], "ended");
    assert_eq!(ended["ended_at"], "2026-11-01T10:50:15Z");
    assert_eq!(ended["duration_sec"], 50 * 60 + 15);
    assert_eq!(
        ended["room_token"],
        Value::Null,
        "終了後はトークンを返さない"
    );
    assert!(
        ended.get("packet_loss_rate").is_none(),
        "指標はレスポンスに載せない"
    );

    // call.ended に通話基盤の 4 属性が載る
    let events = app.call_events();
    assert_eq!(events.len(), 2, "{events:?}");
    assert_eq!(
        events[1],
        AnalyticsEvent::CallEnded {
            student_id: account_id(STUDENT),
            coach_id: account_id(COACH),
            booking_id: booking,
            session_id: session,
            duration_sec: 50 * 60 + 15,
            packet_loss_rate: 3.25,
            jitter_ms: 48,
            round_trip_ms: 210,
            disconnect_count: 1,
            occurred_at: ended_at,
        }
    );
    assert_eq!(events[1].name(), "call.ended");

    // 2 回目の終了は 409 で、イベントは増えない
    let (status, body) = app.end(STUDENT, &session_id).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "invalid_session_state");
    assert_eq!(app.call_events().len(), 2);
}

#[tokio::test]
async fn non_participants_and_unknown_ids_are_not_found() {
    let app = app();
    let booking_id = app.confirmed_booking().await;
    app.clock.set(nov1_10());

    // 予約の参加者でなければ、予約があることも知らせない
    for token in [OTHER_STUDENT, OTHER_COACH, HEADQUARTERS] {
        let (status, body) = app.start(token, &booking_id).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{token}: {body}");
    }
    let unknown_booking = booking_id_uuid::format(BookingId::from_u128(99));
    let (status, _) = app.start(STUDENT, &unknown_booking).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (_, started) = app.start(STUDENT, &booking_id).await;
    let session_id = started["session_id"].as_str().unwrap();
    for token in [OTHER_STUDENT, OTHER_COACH, HEADQUARTERS] {
        let (status, body) = app.end(token, session_id).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{token}: {body}");
    }
    let unknown_session = session_id_uuid::format(SessionId::from_u128(99));
    for id in [unknown_session.as_str(), "not-a-uuid"] {
        let (status, _) = app.end(STUDENT, id).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}");
    }
    assert_eq!(app.call_events().len(), 1, "call.started の 1 回だけ");
}

#[tokio::test]
async fn malformed_requests_and_missing_tokens_are_rejected() {
    let app = app();

    let (status, body) = app.start(STUDENT, "not-a-uuid").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");

    let unknown_session = session_id_uuid::format(SessionId::from_u128(99));
    for path in [
        "/live-lesson-sessions".to_string(),
        format!("/live-lesson-sessions/{unknown_session}/end"),
    ] {
        let (status, body) = send(
            &app.router,
            post_json(&path, None, json!({ "booking_id": unknown_session })),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}: {body}");
    }
}
