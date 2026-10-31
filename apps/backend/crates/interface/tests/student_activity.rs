//! `GET /students/{student_id}/activity`（admin-web の `getStudentActivity`）の統合テスト。本部には生徒 1 人の表示名・
//! これからの予約・直近の受けたレッスン（最大 20 件）・キャンセルの件数を返すことを OpenAPI のスキーマで検査する。
//! コーチ・生徒は 403、トークンがなければ 401、生徒でない・ない・UUID でない ID は 404。

use std::time::{Duration, SystemTime};

use application::{
    identity::{
        AccessToken, AccountRepository, GetStudentActivity, InvalidToken, TokenClaims, TokenIssuer,
        TokenVerifier,
    },
    in_memory::{
        InMemoryAccountRepository, InMemoryBookingRepository, InMemoryLessonCompletionRepository,
        InMemorySlotRepository, ManualClock,
    },
    lesson::LessonStudentLessonHistoryLookup,
    profile::ProfileCoachDisplayNameLookup,
    scheduling::{BookingRepository, SchedulingStudentBookingActivityLookup},
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
    lesson::{LessonCompletion, LessonCompletionId, LessonSource},
    live_lesson::SessionId,
    profile::DisplayName,
    scheduling::{Booking, Slot, SlotId, SlotSeats, SlotStatus},
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
const COACH: AccountId = AccountId::from_u128(2);
/// 予約とレッスンのある生徒
const STUDENT: AccountId = AccountId::from_u128(3);
/// 予約もレッスンもない生徒
const NEW_STUDENT: AccountId = AccountId::from_u128(4);
/// アカウントのないコーチ（表示名が引けない）
const GONE_COACH: AccountId = AccountId::from_u128(9);

/// 2026-10-01T00:00:00Z から `hour` 時間後。
fn at(hour: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_812_800 + hour * 3600)
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

fn slot(id: u128, coach: AccountId, hour: u64) -> Slot {
    Slot::restore(
        SlotId::from_u128(id),
        coach,
        Sport::Tennis,
        at(hour),
        at(hour + 1),
        SlotStatus::Open,
        SlotSeats::default(),
    )
    .unwrap()
}

/// 生徒の完了したレッスン。ID が偶数なら同期（通話）、奇数なら非同期（動画添削）。
fn lesson(id: u128, coach: AccountId, hour: u64) -> LessonCompletion {
    let source = if id.is_multiple_of(2) {
        LessonSource::Live(SessionId::from_u128(id))
    } else {
        LessonSource::Async(VideoId::from_u128(id))
    };
    LessonCompletion::restore(
        LessonCompletionId::from_u128(id),
        STUDENT,
        coach,
        source,
        at(hour),
        false,
    )
}

/// 今は 100 時。生徒 `STUDENT` は 200 時の枠（`COACH`）と 150 時の枠（`GONE_COACH`）をこれから受け、50 時の枠は
/// 始まっていて、300 時の枠はキャンセルした。完了したレッスンは 22 件（1〜22 時）。
async fn app() -> Router {
    let accounts = InMemoryAccountRepository::default();
    for account in [
        account(AccountId::from_u128(1), Role::Headquarters),
        account(COACH, Role::Coach),
        account(STUDENT, Role::Student),
        account(NEW_STUDENT, Role::Student),
    ] {
        accounts.insert(account).await.unwrap();
    }
    let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
        slot(20, COACH, 200),
        slot(21, GONE_COACH, 150),
        slot(22, COACH, 50),
        slot(23, COACH, 300),
    ]));
    for slot_id in [20, 21, 22, 23] {
        let booking = bookings
            .reserve(SlotId::from_u128(slot_id), STUDENT, |slot, existing| {
                Booking::reserve(bookings.next_id(), STUDENT, false, slot, existing, at(10))
            })
            .await
            .unwrap();
        if slot_id == 23 {
            bookings
                .update(booking.id(), |booking, slot| {
                    booking.cancel(STUDENT, slot, at(11)).map(|_| ())
                })
                .await
                .unwrap();
        }
    }
    let completions = InMemoryLessonCompletionRepository::with_completions((1..=22).map(|id| {
        let coach = if id == 22 { GONE_COACH } else { COACH };
        lesson(id, coach, id as u64)
    }));
    router(accounts, bookings, completions)
}

fn router(
    accounts: InMemoryAccountRepository,
    bookings: InMemoryBookingRepository,
    completions: InMemoryLessonCompletionRepository,
) -> Router {
    interface::student_activity_router(
        GetStudentActivity::new(
            accounts.clone(),
            SchedulingStudentBookingActivityLookup::new(bookings),
            LessonStudentLessonHistoryLookup::new(completions),
            ProfileCoachDisplayNameLookup::new(accounts),
            ManualClock::new(at(100)),
        ),
        FakeTokens,
    )
}

/// `GET /students/{student_id}/activity`。レスポンスを admin-web の契約で検査する。
async fn get_activity(
    router: &Router,
    student_id: &str,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = Request::get(format!("/students/{student_id}/activity"));
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
        "/students/{student_id}/activity",
        status,
        &body,
    );
    (status, body)
}

fn uuid(id: u128) -> String {
    format!("00000000-0000-0000-0000-{id:012x}")
}

#[tokio::test]
async fn headquarters_sees_upcoming_bookings_recent_lessons_and_cancelled_count() {
    let (status, body) = get_activity(&app().await, &uuid(3), Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["student_id"], uuid(3));
    assert_eq!(body["display_name"], "なまえ3");
    assert_eq!(body["cancelled_booking_count"], 1);

    let upcoming = body["upcoming_bookings"].as_array().unwrap();
    assert_eq!(
        upcoming
            .iter()
            .map(|b| (
                b["booking"]["slot_id"].as_str().unwrap(),
                b["booking"]["status"].as_str().unwrap(),
                b.get("coach_display_name").and_then(Value::as_str),
            ))
            .collect::<Vec<_>>(),
        [
            (uuid(21).as_str(), "pending", None),
            (uuid(20).as_str(), "pending", Some("なまえ2")),
        ],
        "開始の早い順。始まった予約とキャンセルした予約は載せない。コーチが引けなければ表示名を載せない"
    );

    let lessons = body["recent_lessons"].as_array().unwrap();
    assert_eq!(lessons.len(), 20, "最大 20 件");
    assert_eq!(
        lessons[0],
        json!({
            "lesson_id": uuid(22),
            "coach_id": uuid(9),
            "lesson_type": "live_lesson",
            "completed_at": "2026-10-01T22:00:00Z",
        }),
        "コーチが引けなければ表示名を載せない"
    );
    assert_eq!(
        lessons[1],
        json!({
            "lesson_id": uuid(21),
            "coach_id": uuid(2),
            "coach_display_name": "なまえ2",
            "lesson_type": "async_lesson",
            "completed_at": "2026-10-01T21:00:00Z",
        })
    );
    assert_eq!(
        lessons[19]["lesson_id"],
        uuid(3),
        "完了の新しい順で、古い 2 件は載せない"
    );
    assert!(
        !body.to_string().contains("@example.com"),
        "メールアドレスは載せない: {body}"
    );
}

#[tokio::test]
async fn student_without_bookings_or_lessons_is_empty() {
    let (status, body) = get_activity(&app().await, &uuid(4), Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "student_id": uuid(4),
            "display_name": "なまえ4",
            "upcoming_bookings": [],
            "recent_lessons": [],
            "cancelled_booking_count": 0,
        })
    );
}

#[tokio::test]
async fn non_student_missing_and_malformed_ids_are_404() {
    let router = app().await;
    // 1 は本部、2 はコーチ、5 はないアカウント
    for id in [uuid(1), uuid(2), uuid(5), "not-a-uuid".to_string()] {
        let (status, body) = get_activity(&router, &id, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}: {body}");
        assert_eq!(body["code"], "not_found");
    }
}

#[tokio::test]
async fn coach_and_student_are_403_and_no_token_is_401() {
    let router = app().await;
    for token in ["coach:2", "student:3"] {
        let (status, body) = get_activity(&router, &uuid(3), Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
    }

    let (status, body) = get_activity(&router, &uuid(3), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "unauthorized");
}
