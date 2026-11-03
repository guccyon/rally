//! `POST /live-lesson-sessions/{session_id}/headquarters-end`（admin-web の `endLiveLessonSessionByHeadquarters`。本部が
//! 通話セッションを本部の判断で終わらせる）の統合テスト。レスポンスの形を admin-web の OpenAPI のスキーマで検査し、本部だけが
//! 呼べること（403）、終わった時刻が操作の時刻で終わらせた本部のアカウントを返すこと、終わったセッションが終わっていない
//! セッションの一覧から消えること、終了済みが 409・ない ID が 404 であることを確かめる。終了の判定の細部は domain の
//! `LiveLessonSession::end_by_headquarters`、保存はユースケース `EndLiveLessonSessionByHeadquarters` のテスト。

use std::time::{Duration, SystemTime};

use application::{
    identity::{InvalidToken, TokenClaims, TokenVerifier},
    in_memory::{FixedClock, InMemoryAccountRepository, InMemoryLiveLessonSessionRepository},
    live_lesson::{EndLiveLessonSessionByHeadquarters, ListOverdueLiveLessonSessions},
    profile::{ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    identity::{AccountId, Role},
    live_lesson::{ConnectionQuality, LiveLessonSession, SessionId},
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
/// 契約のパス（スキーマの検査に使う）
const PATH: &str = "/live-lesson-sessions/{session_id}/headquarters-end";

/// 2026-11-01T12:00:00Z（本部が操作する時刻）
fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200) + 12 * HOUR
}

fn uuid(id: u128) -> String {
    format!("00000000-0000-0000-0000-{id:012x}")
}

fn path(session_id: &str) -> String {
    format!("/live-lesson-sessions/{session_id}/headquarters-end")
}

/// 生徒 3 の予約（ID は `100 + id`）でコーチ 42 が `started_at` に始めたセッション。`ended_at` があれば終了した。
fn session(id: u128, started_at: SystemTime, ended_at: Option<SystemTime>) -> LiveLessonSession {
    LiveLessonSession::restore(
        SessionId::from_u128(id),
        BookingId::from_u128(100 + id),
        AccountId::from_u128(3),
        AccountId::from_u128(42),
        vec![],
        started_at,
        ended_at.map(|ended_at| (ended_at, ConnectionQuality::new(0.0, 0, 0, 0).unwrap())),
    )
    .unwrap()
}

/// 5 時間前に始めたまま残ったセッション 1 と、終了したセッション 2。終わらせる操作と、終わっていないセッションの一覧を
/// 同じ保存先で組み立てる。
fn app() -> Router {
    let sessions = InMemoryLiveLessonSessionRepository::with_sessions([
        session(1, now() - 5 * HOUR, None),
        session(2, now() - 10 * HOUR, Some(now() - 9 * HOUR)),
    ]);
    let accounts = InMemoryAccountRepository::default();
    interface::headquarters_live_lesson_session_ends_router(
        EndLiveLessonSessionByHeadquarters::new(sessions.clone(), FixedClock(now())),
        FakeTokens,
    )
    .merge(interface::overdue_live_lesson_sessions_router(
        ListOverdueLiveLessonSessions::new(
            sessions,
            ProfileCoachDisplayNameLookup::new(accounts.clone()),
            ProfileStudentDisplayNameLookup::new(accounts),
            FixedClock(now()),
        ),
        FakeTokens,
    ))
}

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn end(app: &Router, session_id: &str, token: &str) -> (StatusCode, Value) {
    send(
        app,
        Request::post(path(session_id))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await
}

async fn overdue(app: &Router) -> Value {
    let (status, body) = send(
        app,
        Request::get("/live-lesson-sessions/overdue")
            .header(header::AUTHORIZATION, format!("Bearer {HEADQUARTERS}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

#[tokio::test]
async fn headquarters_ends_a_session_left_in_progress_at_the_time_of_the_operation() {
    let admin = support::Contract::load("admin-web");
    let app = app();
    assert_eq!(overdue(&app).await["sessions"].as_array().unwrap().len(), 1);

    let (status, body) = end(&app, &uuid(1), HEADQUARTERS).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    admin.assert_response("post", PATH, status, &body);
    assert_eq!(
        body,
        json!({
            "session_id": uuid(1),
            "booking_id": uuid(101),
            "coach_id": uuid(42),
            "student_id": uuid(3),
            "started_at": "2026-11-01T07:00:00Z",
            "ended_at": "2026-11-01T12:00:00Z",
            "ended_by": uuid(1),
        })
    );
    assert_eq!(
        overdue(&app).await["sessions"],
        json!([]),
        "終わらせたセッションは終わっていないセッションの一覧に出ない"
    );
}

#[tokio::test]
async fn an_ended_session_is_409_and_keeps_the_first_end() {
    let admin = support::Contract::load("admin-web");
    let app = app();

    // 参加者が終了したセッション
    let (status, body) = end(&app, &uuid(2), HEADQUARTERS).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    admin.assert_response("post", PATH, status, &body);
    assert_eq!(body["code"], "invalid_session_state");
    assert_eq!(body["message"], "通話セッションは終了済み");

    // 本部が終わらせたセッションの 2 回目（別の本部のアカウントでも）
    let (status, _) = end(&app, &uuid(1), HEADQUARTERS).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = end(&app, &uuid(1), "headquarters:5").await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    admin.assert_response("post", PATH, status, &body);
    assert_eq!(body["code"], "invalid_session_state");
}

#[tokio::test]
async fn a_missing_or_non_uuid_session_is_404() {
    let admin = support::Contract::load("admin-web");
    let app = app();
    for session_id in [uuid(99), "nope".to_string()] {
        let (status, body) = end(&app, &session_id, HEADQUARTERS).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{session_id}: {body}");
        admin.assert_response("post", PATH, status, &body);
        assert_eq!(body["code"], "not_found");
    }
}

#[tokio::test]
async fn only_headquarters_ends_a_session() {
    let admin = support::Contract::load("admin-web");
    let app = app();
    // 参加者のコーチと生徒でも 403（参加者の終了は endLiveLessonSession）
    for token in [COACH, STUDENT] {
        let (status, body) = end(&app, &uuid(1), token).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        admin.assert_response("post", PATH, status, &body);
    }
    let (status, body) = end(&app, &uuid(1), "nope").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    admin.assert_response("post", PATH, status, &body);

    // 何も変えていない
    assert_eq!(overdue(&app).await["sessions"].as_array().unwrap().len(), 1);
}
