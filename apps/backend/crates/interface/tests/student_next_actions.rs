//! `/me/next-actions` の統合テスト。student-app の `GET /me/next-actions`（`getMyNextActions`。ADR 0010）の
//! レスポンスの形を OpenAPI のスキーマで検査し、生徒本人の次の操作だけを契約の順に 3 件まで返すこと・各 `kind` に載る
//! 項目・次の操作がなければ空の配列・生徒以外（コーチ・本部）は 403、トークンがなければ 401 になることを確かめる。

use std::time::{Duration, SystemTime};

use application::{
    async_lesson::{VideoRepository, video_id_uuid},
    catalog::lesson_menu_id_uuid,
    identity::{
        AccessToken, AccountRepository, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier,
    },
    in_memory::{
        FixedClock, InMemoryAccountRepository, InMemoryBookingRepository,
        InMemoryEntitlementRepository, InMemoryLessonCompletionRepository,
        InMemoryLiveLessonSessionRepository, InMemorySlotRepository, InMemoryVideoRepository,
    },
    lesson::LessonCompletionRepository,
    live_lesson::{LiveLessonSessionRepository, session_id_uuid},
    profile::ProfileCoachDisplayNameLookup,
    scheduling::{BookingRepository, GetMyNextActions, booking_id_uuid},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    async_lesson::{ReviewComment, Video, VideoId, VideoReviewId},
    catalog::{LessonDuration, LessonMenuId, LessonMenuName, Sport},
    identity::{Account, AccountId, Email, Role},
    lesson::LessonCompletionId,
    live_lesson::{ConnectionQuality, LiveLessonSession, SessionId},
    profile::DisplayName,
    scheduling::{Booking, BookingId, BookingLessonMenu, Slot, SlotCapacity, SlotId},
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

/// 確定待ちの予約・完了の記録のない動画と通話・添削待ちの動画がある生徒
const STUDENT: &str = "student:1";
/// 確定待ちの予約と添削待ちの動画がある、別の生徒
const OTHER_STUDENT: &str = "student:2";
/// 入室できる予約・添削待ちの動画・完了したレッスンがあり、受講権がない生徒
const SECOND_STUDENT: &str = "student:3";
/// 記録が何もない生徒
const NEW_STUDENT: &str = "student:4";
const COACH: &str = "coach:9";
/// アカウントを置かない（表示名が引けない）コーチ
const UNKNOWN_COACH: &str = "coach:8";
const HEADQUARTERS: &str = "headquarters:5";

fn account_id(token: &str) -> AccountId {
    AccountId::from_u128(token.split_once(':').unwrap().1.parse().unwrap())
}

/// 2026-10-26T00:00:00Z から `hour` 時間後
fn at(hour: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800 + hour * 3600)
}

fn slot(id: u128, coach: &str, hour: u64) -> Slot {
    Slot::publish(
        SlotId::from_u128(id),
        account_id(coach),
        Sport::Tennis,
        at(hour),
        at(hour + 1),
        SlotCapacity::new(1).unwrap(),
    )
    .unwrap()
}

/// `student` が枠 `slot` を予約する（予約 ID は枠と同じ数）。`menu` は予約した時点のメニューの写し。
async fn book(
    bookings: &InMemoryBookingRepository,
    student: &str,
    slot: u128,
    is_trial: bool,
    menu: Option<BookingLessonMenu>,
    confirm: bool,
) -> Booking {
    let student = account_id(student);
    let booking = bookings
        .reserve(SlotId::from_u128(slot), student, |slot, existing| {
            Booking::reserve(
                BookingId::from_u128(slot.id().as_u128()),
                student,
                is_trial,
                slot,
                existing,
                at(0),
            )
            .map(|booking| booking.with_restored_lesson_menu(menu))
        })
        .await
        .unwrap();
    if !confirm {
        return booking;
    }
    bookings
        .update(booking.id(), |booking, slot| {
            booking.confirm(student, slot, at(0))
        })
        .await
        .unwrap()
}

/// `student` が `coach` に `submitted` 時に送った動画 `id`（`returned` があればその時に添削が返った）。
fn video(id: u128, student: &str, coach: &str, submitted: u64, returned: Option<u64>) -> Video {
    let mut video = Video::submit(
        VideoId::from_u128(id),
        account_id(student),
        account_id(coach),
        Sport::Tennis,
        at(submitted),
    );
    if let Some(returned) = returned {
        video
            .return_review(
                VideoReviewId::from_u128(id),
                account_id(coach),
                ReviewComment::new("よい").unwrap(),
                true,
                at(returned),
            )
            .unwrap();
    }
    video
}

/// 今は 20 時。
///
/// - `STUDENT`: 30 時の枠の確定待ちの体験予約 1（メニュー「フォア改善」）、10〜11 時に通話を終えた予約 2（セッション 50）、
///   5 時に添削が返った動画 60、添削待ちの動画 61。どれも完了の記録がない
/// - `OTHER_STUDENT`: 40 時の枠の確定待ちの予約 4、添削待ちの動画 62
/// - `SECOND_STUDENT`: 表示名の引けないコーチの 20 時の枠の確定済みの予約 3、添削待ちの動画 63、完了を記録した動画 64。受講権はない
async fn app() -> Router {
    let accounts = InMemoryAccountRepository::default();
    accounts
        .insert(
            Account::new(
                account_id(COACH),
                Email::new("coach9@example.com").unwrap(),
                DisplayName::new("たなか").unwrap(),
                Role::Coach,
                [Sport::Tennis],
                at(0),
            )
            .unwrap(),
        )
        .await
        .unwrap();

    let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
        slot(1, COACH, 30),
        slot(2, COACH, 10),
        slot(3, UNKNOWN_COACH, 20),
        slot(4, COACH, 40),
    ]));
    let menu = BookingLessonMenu::restore(
        LessonMenuId::from_u128(21),
        LessonMenuName::new("フォア改善").unwrap(),
        Some(LessonDuration::from_minutes(60).unwrap()),
    );
    book(&bookings, STUDENT, 1, true, Some(menu), false).await;
    let finished = book(&bookings, STUDENT, 2, false, None, true).await;
    book(&bookings, SECOND_STUDENT, 3, false, None, true).await;
    book(&bookings, OTHER_STUDENT, 4, false, None, false).await;

    let sessions = InMemoryLiveLessonSessionRepository::default();
    let mut session = LiveLessonSession::start(
        SessionId::from_u128(50),
        account_id(COACH),
        &finished,
        &[],
        None,
        at(10),
    )
    .unwrap();
    session
        .end(
            account_id(COACH),
            at(11),
            ConnectionQuality::new(0.5, 12, 80, 0).unwrap(),
        )
        .unwrap();
    sessions.insert(session).await.unwrap();

    let videos = InMemoryVideoRepository::default();
    let recorded = video(64, SECOND_STUDENT, COACH, 0, Some(1));
    for video in [
        video(60, STUDENT, COACH, 0, Some(5)),
        video(61, STUDENT, COACH, 1, None),
        video(62, OTHER_STUDENT, COACH, 0, None),
        video(63, SECOND_STUDENT, COACH, 2, None),
        recorded.clone(),
    ] {
        videos.insert(video).await.unwrap();
    }

    let completions = InMemoryLessonCompletionRepository::default();
    let second = account_id(SECOND_STUDENT);
    completions
        .record(second, |history| {
            recorded.complete(LessonCompletionId::from_u128(1), second, history, at(2))
        })
        .await
        .unwrap();

    interface::student_next_actions_router(
        GetMyNextActions::new(
            bookings,
            sessions,
            videos,
            completions,
            InMemoryEntitlementRepository::default(),
            ProfileCoachDisplayNameLookup::new(accounts),
            FixedClock(at(20)),
        ),
        FakeTokens,
    )
}

async fn get_my_next_actions(router: &Router, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get("/me/next-actions");
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
    support::Contract::load("student-app").assert_response(
        "get",
        "/me/next-actions",
        status,
        &body,
    );
    (status, body)
}

fn booking_id(id: u128) -> String {
    booking_id_uuid::format(BookingId::from_u128(id))
}

fn video_id(id: u128) -> String {
    video_id_uuid::format(VideoId::from_u128(id))
}

#[tokio::test]
async fn pending_booking_then_unrecorded_lessons_capped_at_three() {
    let router = app().await;
    let (status, body) = get_my_next_actions(&router, Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "next_actions": [
                {
                    "kind": "confirm_booking",
                    "booking_id": booking_id(1),
                    "is_trial": true,
                    "starts_at": "2026-10-27T06:00:00Z",
                    "ends_at": "2026-10-27T07:00:00Z",
                    "sport": "tennis",
                    "lesson_menu": {
                        "lesson_menu_id": lesson_menu_id_uuid::format(LessonMenuId::from_u128(21)),
                        "name": "フォア改善",
                        "duration_minutes": 60
                    },
                    "coach_display_name": "たなか"
                },
                {
                    // 完了の記録のないレッスンは、返った・終わった時刻の古い順
                    "kind": "record_lesson_completion",
                    "lesson_type": "async_lesson",
                    "video_id": video_id(60),
                    "returned_at": "2026-10-26T05:00:00Z",
                    "coach_display_name": "たなか"
                },
                {
                    "kind": "record_lesson_completion",
                    "lesson_type": "live_lesson",
                    "session_id": session_id_uuid::format(SessionId::from_u128(50)),
                    "starts_at": "2026-10-26T10:00:00Z",
                    "ends_at": "2026-10-26T11:00:00Z",
                    "coach_display_name": "たなか"
                }
                // 添削待ちの動画 61 は 4 件目なので返さない
            ]
        })
    );
}

#[tokio::test]
async fn joinable_lesson_then_review_in_progress_then_subscription() {
    let router = app().await;
    let (status, body) = get_my_next_actions(&router, Some(SECOND_STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "next_actions": [
                {
                    // 表示名の引けないコーチは null。メニューのない枠の予約も null
                    "kind": "join_live_lesson",
                    "booking_id": booking_id(3),
                    "is_trial": false,
                    "starts_at": "2026-10-26T20:00:00Z",
                    "ends_at": "2026-10-26T21:00:00Z",
                    "sport": "tennis",
                    "lesson_menu": null,
                    "coach_display_name": null
                },
                {
                    "kind": "video_review_in_progress",
                    "video_id": video_id(63),
                    "submitted_at": "2026-10-26T02:00:00Z",
                    "coach_display_name": "たなか"
                },
                { "kind": "start_subscription" }
            ]
        })
    );
}

#[tokio::test]
async fn other_students_bookings_and_videos_are_not_returned() {
    let router = app().await;
    let (status, body) = get_my_next_actions(&router, Some(OTHER_STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kinds: Vec<(&str, &str)> = body["next_actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|action| {
            (
                action["kind"].as_str().unwrap(),
                action["booking_id"]
                    .as_str()
                    .or(action["video_id"].as_str())
                    .unwrap(),
            )
        })
        .collect();
    let (booking, video) = (booking_id(4), video_id(62));
    assert_eq!(
        kinds,
        [
            ("confirm_booking", booking.as_str()),
            ("video_review_in_progress", video.as_str()),
        ],
        "本人の予約・動画だけ"
    );
}

#[tokio::test]
async fn a_student_without_records_gets_an_empty_list() {
    let router = app().await;
    let (status, body) = get_my_next_actions(&router, Some(NEW_STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "next_actions": [] }));
}

#[tokio::test]
async fn only_students_may_get_next_actions() {
    let router = app().await;
    for token in [COACH, HEADQUARTERS] {
        let (status, body) = get_my_next_actions(&router, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden", "{token}: {body}");
    }
}

#[tokio::test]
async fn getting_without_a_token_is_unauthorized() {
    let router = app().await;
    let (status, body) = get_my_next_actions(&router, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
}
