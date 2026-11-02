//! `GET /students/{student_id}/announcements`（admin-web の `listStudentAnnouncements`）の統合テスト。本部には生徒 1 人が
//! 受け取った本部からのお知らせ（ID・題・送った時刻・既読か・取り下げ済みか）を新しい順に最大 50 件返すことを OpenAPI の
//! スキーマで検査する。コーチ・生徒は 403、トークンがなければ 401、生徒でない・ない・UUID でない ID は 404。

use std::time::{Duration, SystemTime};

use application::{
    identity::{
        AccessToken, AccountRepository, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier,
    },
    in_memory::{
        InMemoryAccountRepository, InMemoryAnnouncementRepository, InMemoryNotificationRepository,
    },
    notification::ListStudentAnnouncements,
    profile::ProfileAnnouncementRecipientLookup,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    identity::{Account, AccountId, Email, Role},
    notification::{
        Announcement, AnnouncementBody, AnnouncementId, AnnouncementTitle, Notification,
        NotificationId, PushNotification,
    },
    profile::DisplayName,
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

const HEADQUARTERS: &str = "headquarters:1";
const HEADQUARTERS_ID: AccountId = AccountId::from_u128(1);
const COACH: AccountId = AccountId::from_u128(2);
/// お知らせを 52 件受け取った生徒
const STUDENT: AccountId = AccountId::from_u128(3);
/// お知らせを受け取っていない生徒
const NEW_STUDENT: AccountId = AccountId::from_u128(4);

/// 2026-10-01T00:00:00Z から `minute` 分後。
fn at(minute: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_812_800 + minute * 60)
}

fn account(id: AccountId, role: Role) -> Account {
    let sports: &[Sport] = match role {
        Role::Headquarters => &[],
        _ => &[Sport::Tennis],
    };
    Account::new(
        id,
        Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
        DisplayName::new(format!("なまえ{}", id.as_u128())).unwrap(),
        role,
        sports.iter().copied(),
        at(0),
    )
    .unwrap()
}

fn announcement(id: u128) -> Announcement {
    Announcement::new(
        AnnouncementId::from_u128(id),
        HEADQUARTERS_ID,
        AnnouncementTitle::new(format!("お知らせ{id}")).unwrap(),
        AnnouncementBody::new("本文は返さない").unwrap(),
        None,
        1,
        at(id as u64),
    )
    .unwrap()
}

/// 生徒 `STUDENT` は 1〜52 分に送ったお知らせ 52 件を受け取り、1〜10 は既読、52 は本部が取り下げた。予約の確定の
/// お知らせも 1 件ある。
async fn app() -> Router {
    let accounts = InMemoryAccountRepository::default();
    for account in [
        account(HEADQUARTERS_ID, Role::Headquarters),
        account(COACH, Role::Coach),
        account(STUDENT, Role::Student),
        account(NEW_STUDENT, Role::Student),
    ] {
        accounts.insert(account).await.unwrap();
    }
    let mut sent: Vec<Announcement> = (1..=52).map(announcement).collect();
    sent[51].withdraw(HEADQUARTERS_ID, at(60)).unwrap();
    let notifications = InMemoryNotificationRepository::with_notifications(
        sent.iter()
            .map(|a| {
                let mut notification = Notification::new(
                    NotificationId::from_u128(100 + a.id().as_u128()),
                    a.push_to(STUDENT),
                    a.sent_at(),
                );
                if a.id().as_u128() <= 10 {
                    notification.mark_read(at(30));
                }
                notification
            })
            .chain([Notification::new(
                NotificationId::from_u128(999),
                PushNotification::BookingConfirmed {
                    student_id: STUDENT,
                    coach_id: COACH,
                    booking_id: BookingId::from_u128(1),
                },
                at(70),
            )]),
    );
    interface::student_announcements_router(
        ListStudentAnnouncements::new(
            notifications,
            InMemoryAnnouncementRepository::with_announcements(sent),
            ProfileAnnouncementRecipientLookup::new(accounts),
        ),
        FakeTokens,
    )
}

/// `GET /students/{student_id}/announcements`。レスポンスを admin-web の契約で検査する。
async fn list(router: &Router, student_id: &str, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get(format!("/students/{student_id}/announcements"));
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
    support::Contract::load("admin-web").assert_response(
        "get",
        "/students/{student_id}/announcements",
        status,
        &body,
    );
    (status, body)
}

fn uuid(id: u128) -> String {
    format!("00000000-0000-0000-0000-{id:012x}")
}

#[tokio::test]
async fn headquarters_sees_the_newest_fifty_announcements_with_read_and_withdrawn() {
    let (status, body) = list(&app().await, &uuid(3), Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let announcements = body["announcements"].as_array().unwrap();
    assert_eq!(announcements.len(), 50, "最大 50 件");
    assert_eq!(
        announcements[0],
        json!({
            "announcement_id": uuid(52),
            "title": "お知らせ52",
            "sent_at": "2026-10-01T00:52:00Z",
            "is_read": false,
            "is_withdrawn": true,
        }),
        "取り下げたものも載る"
    );
    assert_eq!(
        announcements[1],
        json!({
            "announcement_id": uuid(51),
            "title": "お知らせ51",
            "sent_at": "2026-10-01T00:51:00Z",
            "is_read": false,
            "is_withdrawn": false,
        })
    );
    assert_eq!(
        announcements[49],
        json!({
            "announcement_id": uuid(3),
            "title": "お知らせ3",
            "sent_at": "2026-10-01T00:03:00Z",
            "is_read": true,
            "is_withdrawn": false,
        }),
        "送った時刻の新しい順で、古い 2 件は載せない"
    );
    assert!(
        !body.to_string().contains("本文は返さない"),
        "本文は載せない: {body}"
    );
}

#[tokio::test]
async fn student_without_announcements_is_empty() {
    let (status, body) = list(&app().await, &uuid(4), Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "announcements": [] }));
}

#[tokio::test]
async fn non_student_missing_and_malformed_ids_are_404() {
    let router = app().await;
    // 1 は本部、2 はコーチ、5 はないアカウント
    for id in [uuid(1), uuid(2), uuid(5), "not-a-uuid".to_string()] {
        let (status, body) = list(&router, &id, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}: {body}");
        assert_eq!(body["code"], "not_found");
    }
}

#[tokio::test]
async fn coach_and_student_are_403_and_no_token_is_401() {
    let router = app().await;
    for token in ["coach:2", "student:3"] {
        let (status, body) = list(&router, &uuid(3), Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
    }

    let (status, body) = list(&router, &uuid(3), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "unauthorized");
}
