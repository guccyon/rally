//! `GET /live-lesson-sessions`（admin-web の `listLiveLessonSessions`。本部が期間とコーチ・生徒で見る通話セッションの記録の
//! 一覧）の統合テスト。レスポンスの形を admin-web の OpenAPI のスキーマで検査し、本部だけが呼べること（403）、日付
//! （日本時間）の期間に開始したセッションを終了の有無を問わず開始の新しい順に返すこと、終了の時刻と長さ（分）、
//! `coach_id`・`student_id` で絞れること、期間の不正が 422 であることを確かめる。期間の境界は domain の `SessionHistoryCriteria`、
//! 選び方の細部はユースケース `ListLiveLessonSessions` のテスト。

use std::time::{Duration, SystemTime};

use application::{
    identity::{AccountRepository, InvalidToken, TokenClaims, TokenVerifier},
    in_memory::{InMemoryAccountRepository, InMemoryLiveLessonSessionRepository},
    live_lesson::ListLiveLessonSessions,
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
const MINUTE: Duration = Duration::from_secs(60);

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
const PATH: &str = "/live-lesson-sessions";

/// 2026-11-01T00:00:00+09:00（日本時間の 11 月 1 日の始まり）
fn nov1_jst() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_458_800)
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
        // 11 月 1 日の日本時間 0 時ちょうど。59 分 59 秒
        session(
            1,
            3,
            42,
            nov1_jst(),
            Some(nov1_jst() + HOUR - Duration::from_secs(1)),
        ),
        // 10 月 31 日の日本時間 23 時 59 分（期間の外）
        session(
            2,
            3,
            42,
            nov1_jst() - MINUTE,
            Some(nov1_jst() + 30 * MINUTE),
        ),
        // 11 月 2 日の日本時間 10 時。アカウントのないコーチと生徒。通話中
        session(3, 4, 99, nov1_jst() + 34 * HOUR, None),
        // 11 月 1 日の日本時間 19 時から 45 分
        session(
            4,
            3,
            42,
            nov1_jst() + 19 * HOUR,
            Some(nov1_jst() + 19 * HOUR + 45 * MINUTE),
        ),
    ])
}

async fn app() -> Router {
    let accounts = accounts().await;
    interface::headquarters_live_lesson_sessions_router(
        ListLiveLessonSessions::new(
            sessions(),
            ProfileCoachDisplayNameLookup::new(accounts.clone()),
            ProfileStudentDisplayNameLookup::new(accounts),
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
async fn headquarters_lists_sessions_started_in_the_period_newest_first() {
    let admin = support::Contract::load("admin-web");
    let app = app().await;

    let (status, body) = get(
        &app,
        &format!("{PATH}?from=2026-11-01&to=2026-11-02"),
        HEADQUARTERS,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    admin.assert_response("get", PATH, status, &body);
    assert_eq!(body["from"], "2026-11-01");
    assert_eq!(body["to"], "2026-11-02");
    assert_eq!(
        session_ids(&body),
        [uuid(3), uuid(4), uuid(1)],
        "前日の日本時間に始まったものは出ない: {body}"
    );
    let rows = body["sessions"].as_array().unwrap();
    // 通話中は終了と長さが null。アカウントのないコーチと生徒は表示名を載せない
    assert_eq!(
        rows[0],
        json!({
            "session_id": uuid(3),
            "booking_id": uuid(103),
            "coach_id": uuid(99),
            "student_id": uuid(4),
            "started_at": "2026-11-02T01:00:00Z",
            "ended_at": null,
            "duration_minutes": null,
        })
    );
    assert_eq!(
        rows[1],
        json!({
            "session_id": uuid(4),
            "booking_id": uuid(104),
            "coach_id": uuid(42),
            "coach_display_name": "たなか",
            "student_id": uuid(3),
            "student_display_name": "あおき",
            "started_at": "2026-11-01T10:00:00Z",
            "ended_at": "2026-11-01T10:45:00Z",
            "duration_minutes": 45,
        })
    );
    assert_eq!(rows[2]["duration_minutes"], 59, "分は切り捨て");
}

#[tokio::test]
async fn coach_id_narrows_the_sessions() {
    let admin = support::Contract::load("admin-web");
    let app = app().await;
    for (coach, expected) in [
        (uuid(42), vec![uuid(4), uuid(1)]),
        (uuid(99), vec![uuid(3)]),
        // コーチでない ID・ない ID は空の一覧
        (uuid(3), vec![]),
        (uuid(777), vec![]),
    ] {
        let (status, body) = get(
            &app,
            &format!("{PATH}?from=2026-10-31&to=2026-11-02&coach_id={coach}"),
            HEADQUARTERS,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{coach}: {body}");
        admin.assert_response("get", PATH, status, &body);
        let mut expected = expected;
        if coach == uuid(42) {
            // 10 月 31 日を含めれば前日に始まった通話も出る
            expected.push(uuid(2));
        }
        assert_eq!(session_ids(&body), expected, "{coach}");
    }
}

#[tokio::test]
async fn student_id_narrows_the_sessions() {
    let admin = support::Contract::load("admin-web");
    let app = app().await;
    for (query, expected) in [
        (
            format!("student_id={}", uuid(3)),
            vec![uuid(4), uuid(1), uuid(2)],
        ),
        (format!("student_id={}", uuid(4)), vec![uuid(3)]),
        // コーチと組み合わさる
        (
            format!("student_id={}&coach_id={}", uuid(3), uuid(99)),
            vec![],
        ),
        (
            format!("student_id={}&coach_id={}", uuid(4), uuid(99)),
            vec![uuid(3)],
        ),
        // 生徒でない ID・ない ID は空の一覧
        (format!("student_id={}", uuid(42)), vec![]),
        (format!("student_id={}", uuid(777)), vec![]),
    ] {
        let (status, body) = get(
            &app,
            &format!("{PATH}?from=2026-10-31&to=2026-11-02&{query}"),
            HEADQUARTERS,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        admin.assert_response("get", PATH, status, &body);
        assert_eq!(session_ids(&body), expected, "{query}");
    }
    // 期間と組み合わさる（10 月 31 日を外せば前日に始まった通話は出ない）
    let (status, body) = get(
        &app,
        &format!(
            "{PATH}?from=2026-11-01&to=2026-11-02&student_id={}",
            uuid(3)
        ),
        HEADQUARTERS,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(session_ids(&body), [uuid(4), uuid(1)]);
}

#[tokio::test]
async fn only_headquarters_lists_sessions() {
    let admin = support::Contract::load("admin-web");
    let app = app().await;
    let uri = format!("{PATH}?from=2026-11-01&to=2026-11-02");
    for token in [COACH, STUDENT] {
        let (status, body) = get(&app, &uri, token).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        admin.assert_response("get", PATH, status, &body);
    }
    let (status, body) = get(&app, &uri, "nope").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    admin.assert_response("get", PATH, status, &body);
}

#[tokio::test]
async fn invalid_periods_are_422() {
    let admin = support::Contract::load("admin-web");
    let app = app().await;
    for (query, message) in [
        ("to=2026-11-02", "from がない"),
        ("from=2026-11-01", "to がない"),
        ("", "from がない"),
        (
            "from=2026-11-1&to=2026-11-02",
            "from は YYYY-MM-DD の日付でない",
        ),
        (
            "from=2026-11-01&to=2026-02-30",
            "to は YYYY-MM-DD の日付でない",
        ),
        (
            "from=2026-11-02&to=2026-11-01",
            "to は from と同じ日か、それより後の日付にする",
        ),
        (
            "from=2026-11-01&to=2026-12-02",
            "from から to までは 31 日まで",
        ),
        (
            "from=2026-11-01&to=2026-11-02&coach_id=nope",
            "coach_id が UUID でない",
        ),
        (
            "from=2026-11-01&to=2026-11-02&student_id=nope",
            "student_id が UUID でない",
        ),
    ] {
        let (status, body) = get(&app, &format!("{PATH}?{query}"), HEADQUARTERS).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}: {body}");
        admin.assert_response("get", PATH, status, &body);
        assert_eq!(body["code"], "validation_failed", "{query}");
        assert_eq!(body["message"], message, "{query}");
    }
    // 31 日ちょうどは通る
    let (status, body) = get(
        &app,
        &format!("{PATH}?from=2026-11-01&to=2026-12-01"),
        HEADQUARTERS,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}
