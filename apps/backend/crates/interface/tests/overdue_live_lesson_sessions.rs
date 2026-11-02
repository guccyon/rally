//! `GET /live-lesson-sessions/overdue`（admin-web の `listOverdueLiveLessonSessions`。本部が見る、終わっていないまま
//! 時間の過ぎた通話セッションの一覧）の統合テスト。レスポンスの形を admin-web の OpenAPI のスキーマで検査し、本部だけが
//! 呼べること（403）、開始から既定の 3 時間以上たった通話中のセッションを開始の古い順に返すこと、`older_than_hours` で
//! 時間を変えられること、整数でない・範囲の外の `older_than_hours` が 422 であることを確かめる。基準の境界は domain の
//! `OverdueSessionCriteria`、選び方の細部はユースケース `ListOverdueLiveLessonSessions` のテスト。

use std::time::{Duration, SystemTime};

use application::{
    identity::{AccountRepository, InvalidToken, TokenClaims, TokenVerifier},
    in_memory::{FixedClock, InMemoryAccountRepository, InMemoryLiveLessonSessionRepository},
    live_lesson::ListOverdueLiveLessonSessions,
    profile::{ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    identity::{Account, AccountId, Email, Role},
    live_lesson::{ConnectionQuality, LiveLessonSession, SessionId},
    profile::DisplayName,
    scheduling::BookingId,
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
const PATH: &str = "/live-lesson-sessions/overdue";

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

/// 生徒 `student` の予約（ID は `100 + id`）でコーチ `coach` が `started_at` に始めたセッション。`ended_at` があれば終了した。
fn session(
    id: u128,
    student: u128,
    coach: u128,
    started_at: SystemTime,
    ended_at: Option<SystemTime>,
) -> LiveLessonSession {
    LiveLessonSession::restore(
        SessionId::from_u128(id),
        BookingId::from_u128(100 + id),
        AccountId::from_u128(student),
        AccountId::from_u128(coach),
        vec![],
        started_at,
        ended_at.map(|ended_at| (ended_at, ConnectionQuality::new(0.0, 0, 0, 0).unwrap())),
    )
    .unwrap()
}

/// たなか（42）とあおき（3）の通話と、アカウントのないコーチ（99）・生徒（4）の通話。
fn sessions() -> InMemoryLiveLessonSessionRepository {
    InMemoryLiveLessonSessionRepository::with_sessions([
        // 3 時間ちょうど前
        session(1, 3, 42, now() - 3 * HOUR, None),
        // 3 時間に 1 秒足りない
        session(2, 3, 42, now() - 3 * HOUR + Duration::from_secs(1), None),
        // 終了した
        session(3, 3, 42, now() - 10 * HOUR, Some(now() - 9 * HOUR)),
        // 26 時間 30 分前。アカウントのないコーチと生徒
        session(
            4,
            4,
            99,
            now() - 26 * HOUR - Duration::from_secs(30 * 60),
            None,
        ),
        session(5, 3, 42, now() - 5 * HOUR, None),
    ])
}

async fn app() -> Router {
    let accounts = accounts().await;
    interface::overdue_live_lesson_sessions_router(
        ListOverdueLiveLessonSessions::new(
            sessions(),
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

fn session_ids(body: &Value) -> Vec<String> {
    body["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["session_id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn headquarters_lists_sessions_in_progress_for_three_hours_or_more_oldest_first() {
    let admin = support::Contract::load("admin-web");
    let app = app().await;

    let (status, body) = get(&app, PATH, HEADQUARTERS).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    admin.assert_response("get", PATH, status, &body);
    assert_eq!(body["older_than_hours"], 3);
    assert_eq!(
        session_ids(&body),
        [uuid(4), uuid(5), uuid(1)],
        "終了したもの・3 時間に足りないものは出ない: {body}"
    );
    let rows = body["sessions"].as_array().unwrap();
    // アカウントのないコーチと生徒は表示名を載せない
    assert_eq!(
        rows[0],
        json!({
            "session_id": uuid(4),
            "booking_id": uuid(104),
            "coach_id": uuid(99),
            "student_id": uuid(4),
            "started_at": "2026-10-31T09:30:00Z",
            "elapsed_sec": 26 * 3600 + 30 * 60,
        })
    );
    assert_eq!(
        rows[2],
        json!({
            "session_id": uuid(1),
            "booking_id": uuid(101),
            "coach_id": uuid(42),
            "coach_display_name": "たなか",
            "student_id": uuid(3),
            "student_display_name": "あおき",
            "started_at": "2026-11-01T09:00:00Z",
            "elapsed_sec": 3 * 3600,
        })
    );
}

#[tokio::test]
async fn older_than_hours_changes_the_threshold() {
    let admin = support::Contract::load("admin-web");
    let app = app().await;
    for (hours, expected) in [
        (1, vec![uuid(4), uuid(5), uuid(1), uuid(2)]),
        (5, vec![uuid(4), uuid(5)]),
        (27, vec![]),
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
        assert_eq!(session_ids(&body), expected, "{hours} 時間");
    }
}

#[tokio::test]
async fn only_headquarters_lists_overdue_sessions() {
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
        ("0", "開始からの時間は 1〜168 時間"),
        ("169", "開始からの時間は 1〜168 時間"),
        ("-1", "開始からの時間は 1〜168 時間"),
        ("100000", "開始からの時間は 1〜168 時間"),
        ("3.5", "older_than_hours が整数でない"),
        ("three", "older_than_hours が整数でない"),
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
