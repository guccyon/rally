//! `/notifications/me` の統合テスト。student-app と admin-web の `GET /notifications/me`（`listMyNotifications`）と
//! `POST /notifications/me/read`（`markMyNotificationsRead`）のレスポンスの形を、生徒は student-app、コーチは admin-web の
//! OpenAPI のスキーマで検査し、生徒・コーチが自分のお知らせだけを新しい順に種類・本文・作った時刻・既読かで読めること・
//! すべて既読にできて何度呼んでもよいこと・ほかの人のお知らせに触れないこと・本部の 403・トークンなしの 401 を確かめる。

use std::time::{Duration, SystemTime};

use application::{
    identity::{AccessToken, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier},
    in_memory::{FixedClock, InMemoryNotificationRepository},
    notification::{ListMyNotifications, MarkMyNotificationsRead, notification_id_uuid},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    async_lesson::{VideoId, VideoReviewId},
    identity::{AccountId, Role},
    messaging::{MessageId, MessageSender, MessageThreadId},
    notification::{Notification, NotificationId, PushNotification},
    scheduling::BookingId,
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

const STUDENT_ID: AccountId = AccountId::from_u128(1);
const OTHER_STUDENT_ID: AccountId = AccountId::from_u128(2);
const COACH_ID: AccountId = AccountId::from_u128(9);

/// 2026-11-01T00:00:00Z から `minute` 分後
fn at(minute: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200 + minute * 60)
}

fn notification(id: u128, push: PushNotification, minute: u64) -> Notification {
    Notification::new(NotificationId::from_u128(id), push, at(minute))
}

/// 生徒への予約の確定（10 分）・添削の返却（20 分）・コーチからのメッセージ（30 分）と、コーチへの新しい予約（10 分）・
/// 添削の依頼（15 分）・生徒からのメッセージ（25 分）と、ほかの生徒への予約の確定。
fn saved() -> InMemoryNotificationRepository {
    InMemoryNotificationRepository::with_notifications([
        notification(
            1,
            PushNotification::BookingConfirmed {
                student_id: STUDENT_ID,
                coach_id: COACH_ID,
                booking_id: BookingId::from_u128(1),
            },
            10,
        ),
        notification(
            2,
            PushNotification::BookingReceived {
                student_id: STUDENT_ID,
                coach_id: COACH_ID,
                booking_id: BookingId::from_u128(1),
            },
            10,
        ),
        notification(
            3,
            PushNotification::VideoReviewReturned {
                student_id: STUDENT_ID,
                coach_id: COACH_ID,
                video_id: VideoId::from_u128(3),
                review_id: VideoReviewId::from_u128(4),
            },
            20,
        ),
        notification(
            4,
            PushNotification::MessageReceived {
                student_id: STUDENT_ID,
                coach_id: COACH_ID,
                thread_id: MessageThreadId::from_u128(5),
                message_id: MessageId::from_u128(6),
                sender: MessageSender::Coach,
            },
            30,
        ),
        notification(
            5,
            PushNotification::BookingConfirmed {
                student_id: OTHER_STUDENT_ID,
                coach_id: COACH_ID,
                booking_id: BookingId::from_u128(7),
            },
            40,
        ),
        notification(
            6,
            PushNotification::VideoReviewRequested {
                student_id: STUDENT_ID,
                coach_id: COACH_ID,
                video_id: VideoId::from_u128(8),
            },
            15,
        ),
        notification(
            7,
            PushNotification::MessageReceived {
                student_id: STUDENT_ID,
                coach_id: COACH_ID,
                thread_id: MessageThreadId::from_u128(5),
                message_id: MessageId::from_u128(9),
                sender: MessageSender::Student,
            },
            25,
        ),
    ])
}

fn router(notifications: InMemoryNotificationRepository, now: SystemTime) -> Router {
    interface::notifications_router(
        ListMyNotifications::new(notifications.clone()),
        MarkMyNotificationsRead::new(notifications, FixedClock(now)),
        FakeTokens,
    )
}

/// `token` で呼び、レスポンスを呼んだ人のアプリの契約で検査する（コーチは admin-web、それ以外は student-app）。
async fn send(
    router: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap();
    let app = if token.is_some_and(|token| token.starts_with("coach:")) {
        "admin-web"
    } else {
        "student-app"
    };
    support::Contract::load(app).assert_response(&method.to_lowercase(), path, status, &body);
    (status, body)
}

fn id(value: u128) -> String {
    notification_id_uuid::format(NotificationId::from_u128(value))
}

fn mine(read: [bool; 3]) -> Value {
    json!({
        "notifications": [
            {
                "notification_id": id(4),
                "kind": "message_received",
                "body": "コーチからメッセージが届きました",
                "created_at": "2026-11-01T00:30:00Z",
                "is_read": read[0]
            },
            {
                "notification_id": id(3),
                "kind": "video_review_returned",
                "body": "動画の添削が返ってきました",
                "created_at": "2026-11-01T00:20:00Z",
                "is_read": read[1]
            },
            {
                "notification_id": id(1),
                "kind": "booking_confirmed",
                "body": "予約が確定しました",
                "created_at": "2026-11-01T00:10:00Z",
                "is_read": read[2]
            }
        ]
    })
}

/// コーチのお知らせ（新しい順）。`read` は既読か
fn coachs(read: bool) -> Value {
    json!({
        "notifications": [
            {
                "notification_id": id(7),
                "kind": "message_received",
                "body": "生徒からメッセージが届きました",
                "created_at": "2026-11-01T00:25:00Z",
                "is_read": read
            },
            {
                "notification_id": id(6),
                "kind": "video_review_requested",
                "body": "生徒から動画の添削の依頼が届きました",
                "created_at": "2026-11-01T00:15:00Z",
                "is_read": read
            },
            {
                "notification_id": id(2),
                "kind": "booking_confirmed",
                "body": "新しい予約が入りました",
                "created_at": "2026-11-01T00:10:00Z",
                "is_read": read
            }
        ]
    })
}

#[tokio::test]
async fn a_student_lists_their_own_notifications_newest_first() {
    let notifications = saved();
    let router = router(notifications.clone(), at(60));
    let (status, body) = send(&router, "GET", "/notifications/me", Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, mine([false; 3]), "コーチ・ほかの生徒の分は出さない");
    assert!(
        notifications.all().iter().all(|n| !n.is_read()),
        "照会では既読にしない"
    );

    let (status, body) = send(&router, "GET", "/notifications/me", Some("student:3")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "notifications": [] }));
}

#[tokio::test]
async fn a_student_marks_all_their_notifications_read() {
    let notifications = saved();
    let router = router(notifications.clone(), at(60));
    let (status, body) = send(&router, "POST", "/notifications/me/read", Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, mine([true; 3]));

    let (status, listed) = send(&router, "GET", "/notifications/me", Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_eq!(listed, body);

    // 何度呼んでもよく、最初に読んだ時刻のまま
    let (status, again) = send(&router, "POST", "/notifications/me/read", Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again, body);
    let read_at: Vec<_> = notifications
        .all()
        .iter()
        .map(|n| (n.id(), n.read_at()))
        .collect();
    assert_eq!(
        read_at,
        [
            (NotificationId::from_u128(1), Some(at(60))),
            (NotificationId::from_u128(2), None),
            (NotificationId::from_u128(3), Some(at(60))),
            (NotificationId::from_u128(4), Some(at(60))),
            (NotificationId::from_u128(5), None),
            (NotificationId::from_u128(6), None),
            (NotificationId::from_u128(7), None),
        ],
        "コーチ・ほかの生徒のお知らせは既読にしない"
    );

    let (status, body) = send(&router, "GET", "/notifications/me", Some(OTHER_STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["notifications"][0]["is_read"], false);
}

#[tokio::test]
async fn a_coach_lists_their_own_notifications_and_marks_them_read() {
    let notifications = saved();
    let router = router(notifications.clone(), at(60));
    let (status, body) = send(&router, "GET", "/notifications/me", Some(COACH)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        coachs(false),
        "新しい予約・添削の依頼・生徒からのメッセージだけ。生徒の分は出さない"
    );
    assert!(
        notifications.all().iter().all(|n| !n.is_read()),
        "照会では既読にしない"
    );

    let (status, body) = send(&router, "POST", "/notifications/me/read", Some(COACH)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, coachs(true));
    let (status, again) = send(&router, "POST", "/notifications/me/read", Some(COACH)).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again, body, "何度呼んでもよい");
    let read: Vec<_> = notifications
        .all()
        .iter()
        .filter(|n| n.is_read())
        .map(Notification::id)
        .collect();
    assert_eq!(
        read,
        [2, 6, 7].map(NotificationId::from_u128),
        "生徒のお知らせは既読にしない"
    );

    let (status, body) = send(&router, "GET", "/notifications/me", Some("coach:10")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "notifications": [] }));
}

#[tokio::test]
async fn only_students_and_coaches_can_call_and_a_token_is_required() {
    let notifications = saved();
    let router = router(notifications.clone(), at(60));
    for (method, path) in [
        ("GET", "/notifications/me"),
        ("POST", "/notifications/me/read"),
    ] {
        let (status, body) = send(&router, method, path, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}: {body}");
        assert_eq!(body["code"], "forbidden");
        let (status, body) = send(&router, method, path, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}: {body}");
        assert_eq!(body["code"], "unauthorized");
    }
    assert!(
        notifications.all().iter().all(|n| !n.is_read()),
        "断ったときは既読にしない"
    );
}
