//! `GET /video-reviews/pending`（admin-web の `listPendingVideoReviews`）の統合テスト。本部にはコーチがまだ返していない
//! 動画添削をコーチを問わず送付の古い順に、生徒と担当のコーチの ID と表示名・送った時刻・経過時間（時間。端数は切り捨て）
//! とともに返すことを OpenAPI のスキーマで検査する。クエリの `sport` でスポーツを、`min_elapsed_hours` で経過時間の下限を
//! 絞り、値が不正なら 422 `validation_failed`。コーチ・生徒は 403、トークンがなければ 401
//! （経過時間の数え方の境目は domain の `PendingVideoReviewFilter` のテスト）。

use std::time::{Duration, SystemTime};

use application::{
    Unavailable,
    async_lesson::{ListPendingVideoReviews, UpdateVideoError, VideoRepository},
    identity::{
        AccessToken, AccountRepository, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier,
    },
    in_memory::{FixedClock, InMemoryAccountRepository, InMemoryVideoRepository},
    profile::{ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    DomainError,
    async_lesson::{ReviewComment, Video, VideoId, VideoReviewId},
    catalog::Sport,
    identity::{Account, AccountId, Email, Role},
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

const HEADQUARTERS: &str = "headquarters:1";
const COACH: AccountId = AccountId::from_u128(2);
const OTHER_COACH: AccountId = AccountId::from_u128(3);
const STUDENT: AccountId = AccountId::from_u128(4);
/// アカウントのない生徒（表示名が引けない）
const UNKNOWN_STUDENT: AccountId = AccountId::from_u128(5);
const HOUR: Duration = Duration::from_secs(3600);

/// 照会する時刻（2026-10-31T00:00:00Z）。
fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
}

/// 動画の保存先が障害を返す偽物。
#[derive(Clone)]
struct BrokenVideos;

impl VideoRepository for BrokenVideos {
    fn next_video_id(&self) -> VideoId {
        VideoId::from_u128(0)
    }

    fn next_review_id(&self) -> VideoReviewId {
        VideoReviewId::from_u128(0)
    }

    async fn find(&self, _video_id: VideoId) -> Result<Option<Video>, Unavailable> {
        unreachable!("一覧では読まない")
    }

    async fn videos_of_coach(&self, _coach_id: AccountId) -> Result<Vec<Video>, Unavailable> {
        unreachable!("一覧では読まない")
    }

    async fn videos_of_student(&self, _student_id: AccountId) -> Result<Vec<Video>, Unavailable> {
        unreachable!("一覧では読まない")
    }

    async fn videos_awaiting_review(&self) -> Result<Vec<Video>, Unavailable> {
        Err(Unavailable::new(
            "connection to postgres://rally:secret@db.internal:5432 refused",
        ))
    }

    async fn videos_returned_between(
        &self,
        _starts_at: SystemTime,
        _ends_at: SystemTime,
    ) -> Result<Vec<Video>, Unavailable> {
        unreachable!("返却待ちの一覧では読まない")
    }

    async fn insert(&self, _video: Video) -> Result<(), Unavailable> {
        unreachable!("一覧では保存しない")
    }

    async fn update<F>(&self, _video_id: VideoId, _update: F) -> Result<Video, UpdateVideoError>
    where
        F: FnOnce(&mut Video) -> Result<(), DomainError> + Send,
    {
        unreachable!("一覧では保存しない")
    }
}

fn account(id: AccountId, name: &str, role: Role) -> Account {
    Account::new(
        id,
        Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
        DisplayName::new(name).unwrap(),
        role,
        if role == Role::Headquarters {
            vec![]
        } else {
            vec![Sport::Tennis]
        },
        SystemTime::UNIX_EPOCH,
    )
    .unwrap()
}

/// `id` の動画を `student` が `coach` に `sport` で `submitted_at` に送る。`returned` なら照会の時刻に返却済み。
fn video(
    id: u128,
    student: AccountId,
    coach: AccountId,
    sport: Sport,
    submitted_at: SystemTime,
    returned: bool,
) -> Video {
    Video::restore(
        VideoId::from_u128(id),
        student,
        coach,
        sport,
        submitted_at,
        returned.then(|| {
            (
                VideoReviewId::from_u128(id + 100),
                ReviewComment::new("テイクバックが早い").unwrap(),
                now(),
            )
        }),
        None,
    )
    .unwrap()
}

fn router<V>(accounts: InMemoryAccountRepository, videos: V) -> Router
where
    V: VideoRepository + 'static,
{
    interface::pending_video_reviews_router(
        ListPendingVideoReviews::new(
            videos,
            ProfileCoachDisplayNameLookup::new(accounts.clone()),
            ProfileStudentDisplayNameLookup::new(accounts),
            FixedClock(now()),
        ),
        FakeTokens,
    )
}

async fn app() -> Router {
    let accounts = InMemoryAccountRepository::default();
    for account in [
        account(AccountId::from_u128(1), "本部", Role::Headquarters),
        account(COACH, "たなかコーチ", Role::Coach),
        account(OTHER_COACH, "いとうコーチ", Role::Coach),
        account(STUDENT, "あおき", Role::Student),
    ] {
        accounts.insert(account).await.unwrap();
    }
    router(
        accounts,
        InMemoryVideoRepository::with_videos([
            // 48 時間ちょうど前
            video(10, STUDENT, COACH, Sport::Tennis, now() - 48 * HOUR, false),
            // 72 時間 30 分前（経過時間は 72）。生徒のアカウントが引けない
            video(
                11,
                UNKNOWN_STUDENT,
                OTHER_COACH,
                Sport::Golf,
                now() - 72 * HOUR - HOUR / 2,
                false,
            ),
            // 47 時間 59 分 59 秒前（経過時間は 47）
            video(
                12,
                STUDENT,
                OTHER_COACH,
                Sport::Tennis,
                now() - 48 * HOUR + Duration::from_secs(1),
                false,
            ),
            // 返却済みは載らない
            video(13, STUDENT, COACH, Sport::Tennis, now() - 200 * HOUR, true),
        ]),
    )
}

/// `GET /video-reviews/pending` にクエリ `query`（`?` から。なければ空）を付ける。レスポンスを admin-web の契約で検査する。
async fn list(router: &Router, token: Option<&str>, query: &str) -> (StatusCode, Value) {
    let mut request = Request::get(format!("/video-reviews/pending{query}"));
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
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    if status != StatusCode::INTERNAL_SERVER_ERROR {
        support::Contract::load("admin-web").assert_response(
            "get",
            "/video-reviews/pending",
            status,
            &body,
        );
    }
    (status, body)
}

fn ids(body: &Value) -> Vec<String> {
    body["videos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|video| video["video_id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn headquarters_lists_pending_video_reviews_oldest_first() {
    let (status, body) = list(&app().await, Some(HEADQUARTERS), "").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "as_of": "2026-10-31T00:00:00Z",
            "videos": [
                {
                    "video_id": "00000000-0000-0000-0000-00000000000b",
                    "sport": "golf",
                    // 生徒のアカウントが引けないので student_display_name は載らない
                    "student_id": "00000000-0000-0000-0000-000000000005",
                    "coach_id": "00000000-0000-0000-0000-000000000003",
                    "coach_display_name": "いとうコーチ",
                    "submitted_at": "2026-10-27T23:30:00Z",
                    "elapsed_hours": 72,
                },
                {
                    "video_id": "00000000-0000-0000-0000-00000000000a",
                    "sport": "tennis",
                    "student_id": "00000000-0000-0000-0000-000000000004",
                    "student_display_name": "あおき",
                    "coach_id": "00000000-0000-0000-0000-000000000002",
                    "coach_display_name": "たなかコーチ",
                    "submitted_at": "2026-10-29T00:00:00Z",
                    "elapsed_hours": 48,
                },
                {
                    "video_id": "00000000-0000-0000-0000-00000000000c",
                    "sport": "tennis",
                    "student_id": "00000000-0000-0000-0000-000000000004",
                    "student_display_name": "あおき",
                    "coach_id": "00000000-0000-0000-0000-000000000003",
                    "coach_display_name": "いとうコーチ",
                    "submitted_at": "2026-10-29T00:00:01Z",
                    "elapsed_hours": 47,
                },
            ],
        }),
        "返却済み（13）は載らない"
    );
}

#[tokio::test]
async fn sport_and_min_elapsed_hours_narrow_the_list() {
    let router = app().await;
    for (query, expected) in [
        (
            "?min_elapsed_hours=48",
            vec![
                "00000000-0000-0000-0000-00000000000b",
                "00000000-0000-0000-0000-00000000000a",
            ],
        ),
        ("?min_elapsed_hours=73", vec![]),
        (
            "?min_elapsed_hours=0",
            vec![
                "00000000-0000-0000-0000-00000000000b",
                "00000000-0000-0000-0000-00000000000a",
                "00000000-0000-0000-0000-00000000000c",
            ],
        ),
        (
            "?sport=tennis",
            vec![
                "00000000-0000-0000-0000-00000000000a",
                "00000000-0000-0000-0000-00000000000c",
            ],
        ),
        (
            "?sport=tennis&min_elapsed_hours=48",
            vec!["00000000-0000-0000-0000-00000000000a"],
        ),
        ("?sport=golf", vec!["00000000-0000-0000-0000-00000000000b"]),
        ("?sport=soccer", vec![]),
    ] {
        let (status, body) = list(&router, Some(HEADQUARTERS), query).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(ids(&body), expected, "{query}");
    }
}

#[tokio::test]
async fn invalid_query_is_422_validation_failed() {
    let router = app().await;
    for query in [
        "?sport=pingpong",
        "?sport=",
        "?sport=Tennis",
        "?min_elapsed_hours=-1",
        "?min_elapsed_hours=1.5",
        "?min_elapsed_hours=abc",
        "?min_elapsed_hours=",
        "?min_elapsed_hours=2147483648",
    ] {
        let (status, body) = list(&router, Some(HEADQUARTERS), query).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}: {body}");
        assert_eq!(body["code"], "validation_failed", "{query}");
    }
    let (status, body) = list(&router, Some(HEADQUARTERS), "?min_elapsed_hours=2147483647").await;
    assert_eq!(status, StatusCode::OK, "上限ちょうどは受け付ける: {body}");
    assert_eq!(body["videos"], json!([]));
}

#[tokio::test]
async fn no_pending_videos_is_an_empty_list() {
    let router = router(
        InMemoryAccountRepository::default(),
        InMemoryVideoRepository::with_videos([video(
            1,
            STUDENT,
            COACH,
            Sport::Tennis,
            now() - HOUR,
            true,
        )]),
    );
    let (status, body) = list(&router, Some(HEADQUARTERS), "").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "as_of": "2026-10-31T00:00:00Z", "videos": [] })
    );
}

#[tokio::test]
async fn coach_and_student_are_403_and_no_token_is_401() {
    let router = app().await;
    for token in ["coach:2", "student:4"] {
        let (status, body) = list(&router, Some(token), "").await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
    }

    let (status, body) = list(&router, None, "").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "unauthorized");
}

#[tokio::test]
async fn repository_failure_is_500_internal_without_the_reason() {
    // 契約は 500 の応答を書かないので、形は検査しない
    let (status, body) = list(
        &router(InMemoryAccountRepository::default(), BrokenVideos),
        Some(HEADQUARTERS),
        "",
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert_eq!(body["code"], "internal");
    assert!(!body.to_string().contains("secret"), "{body}");
}
