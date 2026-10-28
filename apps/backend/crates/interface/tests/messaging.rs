//! `/message-threads` の統合テスト。student-app（生徒）と admin-web（コーチ）の `listMessageThreads`・
//! `listMessages`・`sendMessage`・`markMessageThreadRead` のレスポンスの形を OpenAPI のスキーマで検査し、
//! 送信 → 一覧 → 既読の一連・行動イベント `message.sent`・月額の生徒が送り放題で受講権を消費しないこと・
//! 受講権のない生徒の 403・当事者でないアカウントの 404・422 / 401 を確かめる。

use std::time::{Duration, SystemTime};

use application::{
    analytics::AnalyticsEvent,
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo},
    entitlement::{
        EntitlementChatGate, EntitlementQuotaGate, EntitlementReviewGate, GetEntitlement,
        StartSubscription,
    },
    identity::{
        AccessToken, InvalidToken, RegisterAccount, TokenClaims, TokenIssuer, TokenVerifier,
        account_id_uuid,
    },
    in_memory::{
        FixedVideoCallProvider, FixedVideoStorage, InMemoryAccountRepository,
        InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemoryLessonCompletionRepository, InMemoryLiveLessonSessionRepository,
        InMemoryMessageThreadRepository, InMemorySlotRepository, InMemoryVideoRepository,
        ManualClock, RecordingAnalyticsSink, RecordingDomainEventPublisher,
    },
    lesson::RecordLessonCompletion,
    live_lesson::{EndLiveLessonSession, StartLiveLessonSession},
    messaging::{
        ListMessageThreads, ListMessages, MarkMessageThreadRead, SendMessage,
        message_thread_id_uuid,
    },
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
    entitlement::{Entitlement, EntitlementId, LiveLessonQuota, SubscriptionId, YearMonth},
    identity::{AccountId, Role, Sport},
    messaging::{MessageSender, MessageThread, MessageThreadId},
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

/// 月額契約の生徒
const STUDENT: &str = "student:1";
/// 受講権のない（月額契約の前の）生徒
const TRIAL_STUDENT: &str = "student:2";
const COACH: &str = "coach:9";
const OTHER_COACH: &str = "coach:8";
const HEADQUARTERS: &str = "headquarters:5";

/// 生徒 1 とコーチ 9 のスレッド
const THREAD: MessageThreadId = MessageThreadId::from_u128(100);
/// 生徒 2 とコーチ 9 のスレッド
const TRIAL_THREAD: MessageThreadId = MessageThreadId::from_u128(101);

fn account_id(token: &str) -> AccountId {
    AccountId::from_u128(token.split_once(':').unwrap().1.parse().unwrap())
}

/// 2026-10-26T10:00:00Z（スレッドを開いた時刻）
fn opened_at() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800 + 10 * 3600)
}

fn minutes(n: u64) -> SystemTime {
    opened_at() + Duration::from_secs(n * 60)
}

struct App {
    router: Router,
    clock: ManualClock,
    analytics: RecordingAnalyticsSink,
    entitlements: InMemoryEntitlementRepository,
}

/// 生徒 1（月額）とコーチ 9 のスレッド 100、生徒 2（受講権なし）とコーチ 9 のスレッド 101 を置く。
/// 現在時刻はスレッドを開いた 2026-10-26 10:00 の 1 分後。
fn app() -> App {
    let analytics = RecordingAnalyticsSink::default();
    let events = RecordingDomainEventPublisher::default();
    let clock = ManualClock::new(minutes(1));
    let slots = InMemorySlotRepository::default();
    let bookings = InMemoryBookingRepository::new(slots.clone());
    let entitlements = InMemoryEntitlementRepository::with_entitlements([Entitlement::monthly(
        EntitlementId::from_u128(30),
        account_id(STUDENT),
        Sport::Tennis,
        SubscriptionId::from_u128(31),
        LiveLessonQuota::fresh(YearMonth::new(2026, 10).unwrap()),
    )
    .unwrap()]);
    let videos = InMemoryVideoRepository::default();
    let threads = InMemoryMessageThreadRepository::with_threads([
        MessageThread::open(THREAD, account_id(STUDENT), account_id(COACH), opened_at()).unwrap(),
        MessageThread::open(
            TRIAL_THREAD,
            account_id(TRIAL_STUDENT),
            account_id(COACH),
            opened_at(),
        )
        .unwrap(),
    ]);
    let router = interface::router_with(
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            analytics.clone(),
            clock.clone(),
        ),
        application::identity::ListCoaches::new(InMemoryAccountRepository::default()),
        application::identity::GetAccount::new(InMemoryAccountRepository::default()),
        application::identity::UpdateCoachProfile::new(InMemoryAccountRepository::default()),
        PublishSlot::new(slots.clone(), analytics.clone(), clock.clone()),
        SearchSlots::new(
            slots,
            application::identity::IdentityCoachIntroductionLookup::new(
                InMemoryAccountRepository::default(),
            ),
            clock.clone(),
        ),
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
        GetSlotSupply::new(bookings.clone(), clock.clone()),
        StartSubscription::new(
            entitlements.clone(),
            analytics.clone(),
            events.clone(),
            clock.clone(),
        ),
        GetEntitlement::new(entitlements.clone(), clock.clone()),
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
            FixedVideoStorage,
            analytics.clone(),
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
        RecordLessonCompletion::new(
            InMemoryLiveLessonSessionRepository::default(),
            videos,
            InMemoryLessonCompletionRepository::default(),
            analytics.clone(),
            events.clone(),
            clock.clone(),
        ),
        ListMessageThreads::new(threads.clone()),
        ListMessages::new(threads.clone()),
        SendMessage::new(
            threads.clone(),
            EntitlementChatGate::new(entitlements.clone()),
            analytics.clone(),
            events.clone(),
            clock.clone(),
        ),
        MarkMessageThreadRead::new(threads, clock.clone()),
        application::review::SubmitLessonReview::new(
            application::lesson::LessonCompletionLookup::new(
                application::in_memory::InMemoryLessonCompletionRepository::default(),
            ),
            application::in_memory::InMemoryLessonReviewRepository::default(),
            analytics.clone(),
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
        router,
        clock,
        analytics,
        entitlements,
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

/// 生徒は student-app、それ以外（コーチ・本部）は admin-web の契約で検査する。
fn contract_of(token: Option<&str>) -> support::Contract {
    match token {
        Some(token) if token.starts_with("student") => support::Contract::load("student-app"),
        Some(_) => support::Contract::load("admin-web"),
        // 401 はどちらの契約でも同じ形
        None => support::Contract::load("student-app"),
    }
}

fn thread_path(thread_id: MessageThreadId) -> String {
    message_thread_id_uuid::format(thread_id)
}

impl App {
    async fn list_threads(&self, token: Option<&str>) -> (StatusCode, Value) {
        let (status, body) = send(
            &self.router,
            with_token(Request::get("/message-threads"), token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        contract_of(token).assert_response("get", "/message-threads", status, &body);
        (status, body)
    }

    async fn list_messages(&self, token: Option<&str>, thread_id: &str) -> (StatusCode, Value) {
        let (status, body) = send(
            &self.router,
            with_token(
                Request::get(format!("/message-threads/{thread_id}/messages")),
                token,
            )
            .body(Body::empty())
            .unwrap(),
        )
        .await;
        contract_of(token).assert_response(
            "get",
            "/message-threads/{thread_id}/messages",
            status,
            &body,
        );
        (status, body)
    }

    async fn send_message(
        &self,
        token: Option<&str>,
        thread_id: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let (status, body) = send(
            &self.router,
            with_token(
                Request::post(format!("/message-threads/{thread_id}/messages")),
                token,
            )
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
        )
        .await;
        contract_of(token).assert_response(
            "post",
            "/message-threads/{thread_id}/messages",
            status,
            &body,
        );
        (status, body)
    }

    async fn mark_read(&self, token: Option<&str>, thread_id: &str) -> (StatusCode, Value) {
        let (status, body) = send(
            &self.router,
            with_token(
                Request::post(format!("/message-threads/{thread_id}/read")),
                token,
            )
            .body(Body::empty())
            .unwrap(),
        )
        .await;
        contract_of(token).assert_response(
            "post",
            "/message-threads/{thread_id}/read",
            status,
            &body,
        );
        (status, body)
    }

    /// `token` がスレッド `thread_id` に `text` を送る（201 を確かめる）。
    async fn sent(&self, token: &str, thread_id: MessageThreadId, text: &str) -> Value {
        let (status, message) = self
            .send_message(
                Some(token),
                &thread_path(thread_id),
                json!({ "body": text }),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{message}");
        message
    }

    fn message_events(&self) -> Vec<AnalyticsEvent> {
        self.analytics
            .events()
            .into_iter()
            .filter(|event| event.name() == "message.sent")
            .collect()
    }
}

#[tokio::test]
async fn the_student_and_the_coach_exchange_messages_and_read_them() {
    let app = app();
    let thread = thread_path(THREAD);

    // 開いた直後はメッセージがない
    let (status, body) = app.list_threads(Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["threads"].as_array().unwrap().len(), 1);
    let summary = &body["threads"][0];
    assert_eq!(summary["thread_id"], thread);
    assert_eq!(
        summary["student_id"],
        account_id_uuid::format(account_id(STUDENT))
    );
    assert_eq!(
        summary["coach_id"],
        account_id_uuid::format(account_id(COACH))
    );
    assert_eq!(summary["opened_at"], "2026-10-26T10:00:00Z");
    assert_eq!(summary["last_message"], Value::Null);
    assert_eq!(summary["unread_count"], 0);

    // 生徒が送る
    let question = app
        .sent(STUDENT, THREAD, "サーブのトスが安定しません")
        .await;
    assert_eq!(question["thread_id"], thread);
    assert_eq!(
        question["sender_id"],
        account_id_uuid::format(account_id(STUDENT))
    );
    assert_eq!(question["sender_role"], "student");
    assert_eq!(question["body"], "サーブのトスが安定しません");
    assert_eq!(question["sent_at"], "2026-10-26T10:01:00Z");
    assert_eq!(question["read_at"], Value::Null);

    // コーチの一覧では未読 1
    let (status, body) = app.list_threads(Some(COACH)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let threads = body["threads"].as_array().unwrap();
    assert_eq!(
        threads[0]["thread_id"], thread,
        "最後の出来事が新しいスレッドが先"
    );
    assert_eq!(threads[0]["unread_count"], 1);
    assert_eq!(
        threads[0]["last_message"]["message_id"],
        question["message_id"]
    );
    assert_eq!(threads[1]["thread_id"], thread_path(TRIAL_THREAD));

    // コーチが既読にして返信する
    app.clock.set(minutes(5));
    let (status, read) = app.mark_read(Some(COACH), &thread).await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(read["unread_count"], 0);
    assert_eq!(read["last_message"]["read_at"], "2026-10-26T10:05:00Z");
    let answer = app
        .sent(COACH, THREAD, "トスを上げる手を最後まで伸ばしましょう")
        .await;
    assert_eq!(answer["sender_role"], "coach");

    // 生徒が読むと、送った順に並び、自分のメッセージには既読が付いている
    let (status, body) = app.list_messages(Some(STUDENT), &thread).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["thread_id"], thread);
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(
        messages
            .iter()
            .map(|m| m["message_id"].clone())
            .collect::<Vec<_>>(),
        [question["message_id"].clone(), answer["message_id"].clone()]
    );
    assert_eq!(messages[0]["read_at"], "2026-10-26T10:05:00Z");
    assert_eq!(
        messages[1]["read_at"],
        Value::Null,
        "一覧を読んでも既読にはしない"
    );
    let (_, body) = app.list_threads(Some(STUDENT)).await;
    assert_eq!(body["threads"][0]["unread_count"], 1);

    // 行動イベント message.sent が送信ごとに 1 回
    assert_eq!(
        app.message_events(),
        vec![
            AnalyticsEvent::MessageSent {
                student_id: account_id(STUDENT),
                coach_id: account_id(COACH),
                thread_id: THREAD,
                message_id: application::messaging::message_id_uuid::parse(
                    question["message_id"].as_str().unwrap()
                )
                .unwrap(),
                sender_role: MessageSender::Student,
                occurred_at: minutes(1),
            },
            AnalyticsEvent::MessageSent {
                student_id: account_id(STUDENT),
                coach_id: account_id(COACH),
                thread_id: THREAD,
                message_id: application::messaging::message_id_uuid::parse(
                    answer["message_id"].as_str().unwrap()
                )
                .unwrap(),
                sender_role: MessageSender::Coach,
                occurred_at: minutes(5),
            },
        ]
    );
}

#[tokio::test]
async fn monthly_students_send_as_many_messages_as_they_like_without_consuming_the_entitlement() {
    let app = app();
    let before = app.entitlements.all();
    for i in 0..10 {
        app.sent(STUDENT, THREAD, &format!("質問 {i}")).await;
    }
    assert_eq!(app.message_events().len(), 10);
    assert_eq!(app.entitlements.all(), before, "受講権は何も変わらない");
    let (_, body) = app.list_messages(Some(STUDENT), &thread_path(THREAD)).await;
    assert_eq!(body["messages"].as_array().unwrap().len(), 10);
}

#[tokio::test]
async fn students_without_monthly_cannot_send_but_their_coach_can() {
    let app = app();
    let (status, body) = app
        .send_message(
            Some(TRIAL_STUDENT),
            &thread_path(TRIAL_THREAD),
            json!({ "body": "体験の後に質問です" }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["code"], "chat_not_entitled");

    let message = app.sent(COACH, TRIAL_THREAD, "体験はいかがでしたか").await;
    assert_eq!(message["sender_role"], "coach");
    // 生徒は読める・既読にできる
    let (status, body) = app
        .mark_read(Some(TRIAL_STUDENT), &thread_path(TRIAL_THREAD))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(app.message_events().len(), 1, "拒否した送信は発火しない");
}

#[tokio::test]
async fn others_see_no_thread() {
    let app = app();
    let thread = thread_path(THREAD);
    for token in [TRIAL_STUDENT, OTHER_COACH, HEADQUARTERS] {
        let (status, body) = app.list_messages(Some(token), &thread).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{token}: {body}");
        let (status, body) = app
            .send_message(Some(token), &thread, json!({ "body": "x" }))
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{token}: {body}");
        let (status, body) = app.mark_read(Some(token), &thread).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{token}: {body}");
    }
    // ないスレッド・UUID でない ID も 404
    for id in [
        thread_path(MessageThreadId::from_u128(999)),
        "not-a-uuid".to_string(),
    ] {
        let (status, body) = app.list_messages(Some(STUDENT), &id).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}: {body}");
        let (status, body) = app
            .send_message(Some(STUDENT), &id, json!({ "body": "x" }))
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}: {body}");
        let (status, body) = app.mark_read(Some(STUDENT), &id).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}: {body}");
    }
    // 当事者のスレッドがなければ一覧は空
    for token in [OTHER_COACH, HEADQUARTERS] {
        let (status, body) = app.list_threads(Some(token)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["threads"], json!([]));
    }
    assert!(app.message_events().is_empty());
}

#[tokio::test]
async fn invalid_bodies_are_422() {
    let app = app();
    let thread = thread_path(THREAD);
    for body in [
        json!({ "body": "" }),
        json!({ "body": " \u{3000}" }),
        json!({ "body": "あ".repeat(2001) }),
        json!({}),
        json!({ "body": 1 }),
    ] {
        let (status, response) = app.send_message(Some(STUDENT), &thread, body.clone()).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{body}: {response}"
        );
        assert_eq!(response["code"], "validation_failed");
    }
    // 上限ちょうどは送れる
    app.sent(STUDENT, THREAD, &"あ".repeat(2000)).await;
}

#[tokio::test]
async fn requests_without_a_token_are_401() {
    let app = app();
    let thread = thread_path(THREAD);
    assert_eq!(app.list_threads(None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        app.list_messages(None, &thread).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.send_message(None, &thread, json!({ "body": "x" }))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.mark_read(None, &thread).await.0,
        StatusCode::UNAUTHORIZED
    );
}
