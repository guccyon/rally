//! `/videos/me` の統合テスト。student-app の `GET /videos/me`（`listMyVideos`）のレスポンスの形を OpenAPI のスキーマで
//! 検査し、生徒本人が送った動画だけを返却済みも含めて送った時刻の新しい順に返すこと・各行の項目（送り先のコーチの ID と
//! 表示名・スポーツ・添削の状態・返却された時刻・送った時点のレッスンメニューの写し）・ゴルフの動画を載せないこと・
//! 生徒以外（コーチ・本部）は 403、トークンがなければ 401 になることを確かめる。

use std::time::{Duration, SystemTime};

use application::{
    async_lesson::{ListMyVideos, video_id_uuid},
    catalog::lesson_menu_id_uuid,
    identity::{
        AccessToken, AccountRepository, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier,
        account_id_uuid,
    },
    in_memory::{InMemoryAccountRepository, InMemoryVideoRepository},
    profile::ProfileCoachDisplayNameLookup,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    async_lesson::{ReviewComment, Video, VideoId, VideoLessonMenu, VideoReviewId},
    catalog::{LessonMenuId, LessonMenuName},
    identity::{Account, AccountId, Email, Role, Sport},
    profile::DisplayName,
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
const NEW_STUDENT: &str = "student:3";
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

/// 送った時点のレッスンメニューの写し（ID 50「フォーム添削」）
fn menu_copy() -> VideoLessonMenu {
    VideoLessonMenu::restore(
        LessonMenuId::from_u128(50),
        LessonMenuName::new("フォーム添削").unwrap(),
    )
}

/// `id` の動画を `student` が `coach` に `submitted_hour` に送る。`returned_hour` があればその時刻に返却済み。
fn video(
    id: u128,
    student: &str,
    coach: &str,
    sport: Sport,
    submitted_hour: u64,
    returned_hour: Option<u64>,
) -> Video {
    Video::restore(
        VideoId::from_u128(id),
        account_id(student),
        account_id(coach),
        sport,
        at(submitted_hour),
        returned_hour.map(|hour| {
            (
                VideoReviewId::from_u128(id + 100),
                ReviewComment::new("テイクバックが早い").unwrap(),
                at(hour),
            )
        }),
    )
    .unwrap()
}

/// `STUDENT` が送った動画 1〜3 とゴルフの動画 4、`OTHER_STUDENT` が送った動画 5 を置く。コーチ `COACH` のアカウント
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
    let videos = InMemoryVideoRepository::with_videos([
        // 返却済み・メニューあり（いちばん古い）
        video(1, STUDENT, COACH, Sport::Tennis, 1, Some(30))
            .with_restored_lesson_menu(Some(menu_copy())),
        // 添削待ち・表示名が引けないコーチ（いちばん新しい）
        video(2, STUDENT, UNKNOWN_COACH, Sport::Soccer, 5, None),
        // 添削待ち・メニューなし
        video(3, STUDENT, COACH, Sport::Tennis, 3, None),
        // ゴルフ（golf-app の範囲）は student-app の一覧に載らない
        video(4, STUDENT, COACH, Sport::Golf, 4, None),
        // 別の生徒の動画は載らない
        video(5, OTHER_STUDENT, COACH, Sport::Tennis, 6, None),
    ]);
    interface::my_videos_router(
        ListMyVideos::new(videos, ProfileCoachDisplayNameLookup::new(accounts)),
        FakeTokens,
    )
}

async fn list_my_videos(router: &Router, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get("/videos/me");
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
    support::Contract::load("student-app").assert_response("get", "/videos/me", status, &body);
    (status, body)
}

#[tokio::test]
async fn a_student_lists_own_videos_latest_submission_first_including_returned_ones() {
    let router = app().await;
    let (status, body) = list_my_videos(&router, Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let video_id = |id: u128| video_id_uuid::format(VideoId::from_u128(id));
    let coach_id = |token: &str| account_id_uuid::format(account_id(token));
    assert_eq!(
        body,
        json!({
            "videos": [
                {
                    "video_id": video_id(2),
                    "coach_id": coach_id(UNKNOWN_COACH),
                    "sport": "soccer",
                    "submitted_at": "2026-10-26T05:00:00Z",
                    "review_status": "in_progress",
                    "returned_at": null,
                    "lesson_menu": null
                },
                {
                    "video_id": video_id(3),
                    "coach_id": coach_id(COACH),
                    "coach_display_name": "たなか",
                    "sport": "tennis",
                    "submitted_at": "2026-10-26T03:00:00Z",
                    "review_status": "in_progress",
                    "returned_at": null,
                    "lesson_menu": null
                },
                {
                    "video_id": video_id(1),
                    "coach_id": coach_id(COACH),
                    "coach_display_name": "たなか",
                    "sport": "tennis",
                    "submitted_at": "2026-10-26T01:00:00Z",
                    "review_status": "returned",
                    "returned_at": "2026-10-27T06:00:00Z",
                    "lesson_menu": {
                        "lesson_menu_id": lesson_menu_id_uuid::format(LessonMenuId::from_u128(50)),
                        "name": "フォーム添削"
                    }
                }
            ]
        }),
        "本人の動画だけを送った時刻の新しい順に。ゴルフの動画と別の生徒の動画は載らず、添削の中身も載らない"
    );
}

#[tokio::test]
async fn a_student_without_videos_gets_an_empty_list() {
    let router = app().await;
    let (status, body) = list_my_videos(&router, Some(NEW_STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "videos": [] }));
}

#[tokio::test]
async fn only_students_can_list_their_videos() {
    let router = app().await;
    for token in [COACH, HEADQUARTERS] {
        let (status, body) = list_my_videos(&router, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
    }
    let (status, body) = list_my_videos(&router, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
}
