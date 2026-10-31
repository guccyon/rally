//! `/lessons/completions/me` の統合テスト。student-app の `GET /lessons/completions/me`（`listMyLessonCompletions`）の
//! レスポンスの形を OpenAPI のスキーマで検査し、生徒本人の完了したレッスンだけを通話・動画添削の両方とも完了した時刻の
//! 新しい順に返すこと・各行の項目（コーチの ID と表示名・通話か動画添削か・元の ID・レッスンメニューの写し・評価を
//! 付けたか）・生徒以外（コーチ・本部）は 403、トークンがなければ 401 になることを確かめる。

use std::time::{Duration, SystemTime};

use application::{
    Unavailable,
    async_lesson::video_id_uuid,
    catalog::lesson_menu_id_uuid,
    identity::{
        AccessToken, AccountRepository, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier,
        account_id_uuid,
    },
    in_memory::{
        InMemoryAccountRepository, InMemoryLessonCompletionRepository,
        InMemoryLessonReviewRepository, InMemoryLiveLessonSessionRepository,
        InMemoryVideoRepository,
    },
    lesson::{LessonCompletionRepository, ListMyLessonCompletions, lesson_completion_id_uuid},
    live_lesson::{BookingLookup, session_id_uuid},
    profile::ProfileCoachDisplayNameLookup,
    review::ReviewLessonCompletionReviewLookup,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    async_lesson::{ReviewComment, Video, VideoId, VideoLessonMenu, VideoReviewId},
    catalog::{LessonDuration, LessonMenuId, LessonMenuName, Sport},
    identity::{Account, AccountId, Email, Role},
    lesson::LessonCompletionId,
    live_lesson::{ConnectionQuality, LiveLessonSession, SessionId},
    profile::DisplayName,
    review::{LessonReview, LessonReviewComment, LessonReviewId, Rating},
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

/// 予約の保存先の代わり（読むだけ）。
struct FixedBookings(Vec<Booking>);

impl BookingLookup for FixedBookings {
    async fn find_booking(&self, booking_id: BookingId) -> Result<Option<Booking>, Unavailable> {
        Ok(self.0.iter().find(|b| b.id() == booking_id).cloned())
    }

    async fn find_slot_bookings(&self, booking: &Booking) -> Result<Vec<Booking>, Unavailable> {
        Ok(self
            .0
            .iter()
            .filter(|b| b.slot_id() == booking.slot_id())
            .cloned()
            .collect())
    }

    async fn find_slot(&self, _slot_id: SlotId) -> Result<Option<Slot>, Unavailable> {
        unreachable!("受けたレッスンの一覧では枠を読まない")
    }
}

const STUDENT: &str = "student:1";
const OTHER_STUDENT: &str = "student:2";
const NEW_STUDENT: &str = "student:3";
const COACH: &str = "coach:9";
/// アカウントを置かない（表示名が引けない）コーチ
const UNKNOWN_COACH: &str = "coach:8";
const HEADQUARTERS: &str = "headquarters:5";

/// 生徒 `STUDENT` の完了したレッスン（通話・メニューのある動画・メニューのない動画）と、別の生徒の完了したレッスン
const LIVE_LESSON: u128 = 0x100;
const ASYNC_LESSON: u128 = 0x101;
const ASYNC_LESSON_WITHOUT_MENU: u128 = 0x102;
const OTHER_STUDENTS_LESSON: u128 = 0x103;

fn account_id(token: &str) -> AccountId {
    AccountId::from_u128(token.split_once(':').unwrap().1.parse().unwrap())
}

/// 2026-10-26T00:00:00Z から `hour` 時間後
fn at(hour: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800 + hour * 3600)
}

fn name(text: &str) -> LessonMenuName {
    LessonMenuName::new(text).unwrap()
}

/// `COACH` の枠を `STUDENT` が予約し（メニュー「フォア改善」の写し）、通話して終えたセッション 50。
fn live_lesson() -> (LiveLessonSession, Booking) {
    let mut slot = Slot::publish(
        SlotId::from_u128(30),
        account_id(COACH),
        Sport::Tennis,
        at(10),
        at(11),
        SlotCapacity::new(1).unwrap(),
    )
    .unwrap();
    let mut booking = Booking::reserve(
        BookingId::from_u128(30),
        account_id(STUDENT),
        false,
        &mut slot,
        &[],
        at(0),
    )
    .unwrap()
    .with_restored_lesson_menu(Some(BookingLessonMenu::restore(
        LessonMenuId::from_u128(21),
        name("フォア改善"),
        Some(LessonDuration::from_minutes(60).unwrap()),
    )));
    booking
        .confirm(account_id(STUDENT), &mut slot, at(0))
        .unwrap();
    let mut session = LiveLessonSession::start(
        SessionId::from_u128(50),
        account_id(COACH),
        &booking,
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
    (session, booking)
}

/// `student` が `coach` に送り、返却された動画 `id`（メニューは `menu`）。
fn reviewed_video(id: u128, student: &str, coach: &str, menu: Option<VideoLessonMenu>) -> Video {
    let mut video = Video::restore(
        VideoId::from_u128(id),
        account_id(student),
        account_id(coach),
        Sport::Tennis,
        at(0),
        None,
        menu,
    )
    .unwrap();
    video
        .return_review(
            VideoReviewId::from_u128(id),
            account_id(coach),
            ReviewComment::new("よい").unwrap(),
            true,
            at(1),
        )
        .unwrap();
    video
}

/// `STUDENT` は通話 50 を 14 時、「フォーム添削」の動画 60 を 16 時、表示名の引けないコーチのメニューのない動画 61 を
/// 12 時に完了し、動画 60 にだけ評価を付けた。`OTHER_STUDENT` も動画 62 を完了して評価した。コーチ `COACH` のアカウント
/// （表示名「たなか」）だけを置く。
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
    let (session, booking) = live_lesson();
    let with_menu = reviewed_video(
        60,
        STUDENT,
        COACH,
        Some(VideoLessonMenu::restore(
            LessonMenuId::from_u128(23),
            name("フォーム添削"),
        )),
    );
    let without_menu = reviewed_video(61, STUDENT, UNKNOWN_COACH, None);
    let others = reviewed_video(62, OTHER_STUDENT, COACH, None);

    let student = account_id(STUDENT);
    let lesson = LessonCompletionId::from_u128;
    let completions = InMemoryLessonCompletionRepository::default();
    completions
        .record(student, |history| {
            without_menu.complete(lesson(ASYNC_LESSON_WITHOUT_MENU), student, history, at(12))
        })
        .await
        .unwrap();
    completions
        .record(student, |history| {
            session.complete_for(lesson(LIVE_LESSON), student, student, history, at(14))
        })
        .await
        .unwrap();
    completions
        .record(student, |history| {
            with_menu.complete(lesson(ASYNC_LESSON), student, history, at(16))
        })
        .await
        .unwrap();
    let other = account_id(OTHER_STUDENT);
    completions
        .record(other, |history| {
            others.complete(lesson(OTHER_STUDENTS_LESSON), other, history, at(18))
        })
        .await
        .unwrap();

    let review = |id: u128, lesson_id: u128, student: AccountId| {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            lesson(lesson_id),
            student,
            account_id(COACH),
            Rating::new(5).unwrap(),
            LessonReviewComment::new("よかった").unwrap(),
            at(20),
        )
    };
    let reviews = InMemoryLessonReviewRepository::with_reviews([
        review(1, ASYNC_LESSON, student),
        review(2, OTHER_STUDENTS_LESSON, other),
    ]);

    interface::my_lesson_completions_router(
        ListMyLessonCompletions::new(
            completions,
            InMemoryLiveLessonSessionRepository::with_sessions([session]),
            FixedBookings(vec![booking]),
            InMemoryVideoRepository::with_videos([with_menu, without_menu, others]),
            ProfileCoachDisplayNameLookup::new(accounts),
            ReviewLessonCompletionReviewLookup::new(reviews),
        ),
        FakeTokens,
    )
}

async fn list_my_lesson_completions(router: &Router, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get("/lessons/completions/me");
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
        "/lessons/completions/me",
        status,
        &body,
    );
    (status, body)
}

#[tokio::test]
async fn a_student_lists_own_live_and_async_lessons_latest_completion_first() {
    let router = app().await;
    let (status, body) = list_my_lesson_completions(&router, Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let lesson_id = |id: u128| lesson_completion_id_uuid::format(LessonCompletionId::from_u128(id));
    let coach_id = |token: &str| account_id_uuid::format(account_id(token));
    let menu_id = |id: u128| lesson_menu_id_uuid::format(LessonMenuId::from_u128(id));
    assert_eq!(
        body,
        json!({
            "lesson_completions": [
                {
                    "lesson_id": lesson_id(ASYNC_LESSON),
                    "coach_id": coach_id(COACH),
                    "coach_display_name": "たなか",
                    "lesson_type": "async_lesson",
                    "session_id": null,
                    "video_id": video_id_uuid::format(VideoId::from_u128(60)),
                    "completed_at": "2026-10-26T16:00:00Z",
                    "lesson_menu": {"lesson_menu_id": menu_id(23), "name": "フォーム添削"},
                    "is_reviewed": true
                },
                {
                    "lesson_id": lesson_id(LIVE_LESSON),
                    "coach_id": coach_id(COACH),
                    "coach_display_name": "たなか",
                    "lesson_type": "live_lesson",
                    "session_id": session_id_uuid::format(SessionId::from_u128(50)),
                    "video_id": null,
                    "completed_at": "2026-10-26T14:00:00Z",
                    "lesson_menu": {"lesson_menu_id": menu_id(21), "name": "フォア改善"},
                    "is_reviewed": false
                },
                {
                    // 表示名が引けないコーチは coach_display_name を載せない。メニューのない動画は null
                    "lesson_id": lesson_id(ASYNC_LESSON_WITHOUT_MENU),
                    "coach_id": coach_id(UNKNOWN_COACH),
                    "lesson_type": "async_lesson",
                    "session_id": null,
                    "video_id": video_id_uuid::format(VideoId::from_u128(61)),
                    "completed_at": "2026-10-26T12:00:00Z",
                    "lesson_menu": null,
                    "is_reviewed": false
                }
            ]
        })
    );
}

#[tokio::test]
async fn a_student_without_lessons_gets_an_empty_list() {
    let router = app().await;
    let (status, body) = list_my_lesson_completions(&router, Some(NEW_STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "lesson_completions": [] }));
}

#[tokio::test]
async fn only_students_may_list_their_lessons() {
    let router = app().await;
    for token in [COACH, HEADQUARTERS] {
        let (status, body) = list_my_lesson_completions(&router, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden", "{token}: {body}");
    }
}

#[tokio::test]
async fn listing_without_a_token_is_unauthorized() {
    let router = app().await;
    let (status, body) = list_my_lesson_completions(&router, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
}
