//! `GET /message-threads/unanswered`（admin-web の `listUnansweredMessageThreads`。本部が見る、返信のないまま時間の
//! 過ぎたスレッドの一覧）の統合テスト。レスポンスの形を admin-web の OpenAPI のスキーマで検査し、本部だけが呼べること
//! （403）、最後の発言が生徒で既定の 24 時間以上たったスレッドを経過の長い順に返すこと、メッセージの本文を返さないこと、
//! `older_than_hours` で時間を変えられること、整数でない・範囲の外の `older_than_hours` が 422 であることを確かめる。
//! 何を発言と数えるかと基準の境界は domain の `MessageThread::awaiting_coach_reply_since`・`UnansweredThreadCriteria`、
//! 選び方の細部はユースケース `ListUnansweredMessageThreads` のテスト。

use std::time::{Duration, SystemTime};

use application::{
    identity::{AccountRepository, InvalidToken, TokenClaims, TokenVerifier},
    in_memory::{FixedClock, InMemoryAccountRepository, InMemoryMessageThreadRepository},
    messaging::ListUnansweredMessageThreads,
    profile::{ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    async_lesson::VideoId,
    catalog::Sport,
    identity::{Account, AccountId, Email, Role},
    messaging::{MessageBody, MessageContent, MessageId, MessageThread, MessageThreadId, Stamp},
    profile::DisplayName,
};
use serde_json::{Value, json};
use tower::ServiceExt;

mod support;

const HOUR: Duration = Duration::from_secs(3600);

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
const COACH: &str = "coach:42";
const STUDENT: &str = "student:3";
const PATH: &str = "/message-threads/unanswered";
const SECRET: &str = "本部には見せない本文";

/// 2026-11-01T12:00:00Z（照会する時刻）
fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200) + 12 * HOUR
}

fn uuid(id: u128) -> String {
    format!("00000000-0000-0000-0000-{id:012x}")
}

async fn accounts() -> InMemoryAccountRepository {
    let accounts = InMemoryAccountRepository::default();
    for (id, name, role, sports) in [
        (1, "本部", Role::Headquarters, vec![]),
        (3, "あおき", Role::Student, vec![Sport::Tennis]),
        (42, "たなか", Role::Coach, vec![Sport::Tennis]),
    ] {
        accounts
            .insert(
                Account::new(
                    AccountId::from_u128(id),
                    Email::new(format!("user{id}@example.com")).unwrap(),
                    DisplayName::new(name).unwrap(),
                    role,
                    sports,
                    SystemTime::UNIX_EPOCH,
                )
                .unwrap(),
            )
            .await
            .unwrap();
    }
    accounts
}

/// 生徒 `student` とコーチ `coach` のスレッド `id` を照会の 10 日前に開き、`exchange` の順に
/// （送り手のアカウント, 中身, 送信時刻）でメッセージを送る。
fn thread(
    id: u128,
    student: u128,
    coach: u128,
    exchange: Vec<(u128, MessageContent, SystemTime)>,
) -> MessageThread {
    let mut thread = MessageThread::open(
        MessageThreadId::from_u128(id),
        AccountId::from_u128(student),
        AccountId::from_u128(coach),
        now() - 240 * HOUR,
    )
    .unwrap();
    for (n, (sender, content, at)) in exchange.into_iter().enumerate() {
        thread
            .send(
                MessageId::from_u128(id * 100 + n as u128),
                AccountId::from_u128(sender),
                content,
                true,
                at,
            )
            .unwrap();
    }
    thread
}

fn text() -> MessageContent {
    MessageContent::Text(MessageBody::new(SECRET).unwrap())
}

/// たなか（42）とあおき（3）のスレッドと、アカウントのないコーチ（99）・生徒（4）のスレッド。
fn threads() -> InMemoryMessageThreadRepository {
    let mut with_request = thread(5, 3, 43, vec![]);
    // 添削依頼だけでは返信を待たない
    with_request
        .post_video_review_request(
            MessageId::from_u128(599),
            VideoId::from_u128(1),
            now() - 50 * HOUR,
        )
        .unwrap();
    InMemoryMessageThreadRepository::with_threads([
        // 24 時間ちょうど前の生徒の発言（スタンプ）
        thread(
            1,
            3,
            42,
            vec![
                (42, text(), now() - 30 * HOUR),
                (3, MessageContent::Stamp(Stamp::Question), now() - 24 * HOUR),
            ],
        ),
        // 24 時間に 1 秒足りない
        thread(
            2,
            3,
            44,
            vec![(3, text(), now() - 24 * HOUR + Duration::from_secs(1))],
        ),
        // コーチが返した
        thread(
            3,
            3,
            45,
            vec![
                (3, text(), now() - 90 * HOUR),
                (45, text(), now() - 80 * HOUR),
            ],
        ),
        // 30 時間 30 分前。アカウントのないコーチと生徒
        thread(
            4,
            4,
            99,
            vec![(4, text(), now() - 30 * HOUR - Duration::from_secs(30 * 60))],
        ),
        with_request,
    ])
}

async fn app() -> Router {
    let accounts = accounts().await;
    interface::unanswered_message_threads_router(
        ListUnansweredMessageThreads::new(
            threads(),
            ProfileCoachDisplayNameLookup::new(accounts.clone()),
            ProfileStudentDisplayNameLookup::new(accounts),
            FixedClock(now()),
        ),
        FakeTokens,
    )
}

async fn get(app: &Router, uri: &str, token: &str) -> (StatusCode, Value) {
    let request = Request::get(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn thread_ids(body: &Value) -> Vec<String> {
    body["threads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["thread_id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn headquarters_lists_threads_unanswered_for_twenty_four_hours_or_more_longest_first() {
    let admin = support::Contract::load("admin-web");
    let app = app().await;

    let (status, body) = get(&app, PATH, HEADQUARTERS).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    admin.assert_response("get", PATH, status, &body);
    assert_eq!(body["older_than_hours"], 24);
    assert_eq!(
        thread_ids(&body),
        [uuid(4), uuid(1)],
        "コーチが返したもの・24 時間に足りないもの・添削依頼だけのものは出ない: {body}"
    );
    let rows = body["threads"].as_array().unwrap();
    // アカウントのないコーチと生徒は表示名を載せない
    assert_eq!(
        rows[0],
        json!({
            "thread_id": uuid(4),
            "coach_id": uuid(99),
            "student_id": uuid(4),
            "last_student_message_at": "2026-10-31T05:30:00Z",
            "elapsed_sec": 30 * 3600 + 30 * 60,
        })
    );
    assert_eq!(
        rows[1],
        json!({
            "thread_id": uuid(1),
            "coach_id": uuid(42),
            "coach_display_name": "たなか",
            "student_id": uuid(3),
            "student_display_name": "あおき",
            "last_student_message_at": "2026-10-31T12:00:00Z",
            "elapsed_sec": 24 * 3600,
        })
    );
    assert!(
        !body.to_string().contains(SECRET),
        "メッセージの本文は返さない: {body}"
    );
}

#[tokio::test]
async fn older_than_hours_changes_the_threshold() {
    let admin = support::Contract::load("admin-web");
    let app = app().await;
    for (hours, expected) in [
        (1, vec![uuid(4), uuid(1), uuid(2)]),
        (25, vec![uuid(4)]),
        (31, vec![]),
        (168, vec![]),
    ] {
        let (status, body) = get(
            &app,
            &format!("{PATH}?older_than_hours={hours}"),
            HEADQUARTERS,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{hours}: {body}");
        admin.assert_response("get", PATH, status, &body);
        assert_eq!(body["older_than_hours"], hours);
        assert_eq!(thread_ids(&body), expected, "{hours} 時間");
    }
}

#[tokio::test]
async fn only_headquarters_lists_unanswered_threads() {
    let admin = support::Contract::load("admin-web");
    let app = app().await;
    for token in [COACH, STUDENT] {
        let (status, body) = get(&app, PATH, token).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        admin.assert_response("get", PATH, status, &body);
    }
    let (status, body) = get(&app, PATH, "nope").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    admin.assert_response("get", PATH, status, &body);
}

#[tokio::test]
async fn invalid_older_than_hours_is_422() {
    let admin = support::Contract::load("admin-web");
    let app = app().await;
    for (value, message) in [
        ("0", "生徒の最後の発言からの時間は 1〜168 時間"),
        ("169", "生徒の最後の発言からの時間は 1〜168 時間"),
        ("-1", "生徒の最後の発言からの時間は 1〜168 時間"),
        ("100000", "生徒の最後の発言からの時間は 1〜168 時間"),
        ("24.5", "older_than_hours が整数でない"),
        ("day", "older_than_hours が整数でない"),
        ("", "older_than_hours が整数でない"),
    ] {
        let (status, body) = get(
            &app,
            &format!("{PATH}?older_than_hours={value}"),
            HEADQUARTERS,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{value}: {body}");
        admin.assert_response("get", PATH, status, &body);
        assert_eq!(body["code"], "validation_failed", "{value}");
        assert_eq!(body["message"], message, "{value}");
    }
}
