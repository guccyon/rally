//! `GET /video-reviews/returned`（admin-web の `listReturnedVideoReviews`）の統合テスト。本部にはコーチが返した動画添削を
//! コーチを問わず返却の新しい順に、生徒と担当のコーチの ID と表示名・送った時刻・返した時刻・かかった時間（時間。端数は
//! 切り捨て）とともに返すことを OpenAPI のスキーマで検査する。クエリの `from`・`to`（返却の日付。日本時間）で期間を、
//! `coach_id` でコーチを絞り、値や期間が不正なら 422 `validation_failed`。コーチ・生徒は 403、トークンがなければ 401
//! （かかった時間の数え方の境目は domain の `Video::review_lead_hours` のテスト）。

use std::time::{Duration, SystemTime};

use application::{
    Unavailable,
    async_lesson::{ListReturnedVideoReviews, UpdateVideoError, VideoRepository},
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

/// 照会する時刻（2026-10-31T00:00:00Z。日本時間で 10-31 の 9 時）。
fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
}

/// 日本時間の 2026-10-31 の 0 時（2026-10-30T15:00:00Z）。
fn oct31_jst() -> SystemTime {
    now() - 9 * HOUR
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
        unreachable!("返した記録では読まない")
    }

    async fn videos_returned_between(
        &self,
        _starts_at: SystemTime,
        _ends_at: SystemTime,
    ) -> Result<Vec<Video>, Unavailable> {
        Err(Unavailable::new(
            "connection to postgres://rally:secret@db.internal:5432 refused",
        ))
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

/// `id` の動画を `student` が `coach` に `sport` で `submitted_at` に送る。`returned_at` があればそのときに返却済み。
fn video(
    id: u128,
    student: AccountId,
    coach: AccountId,
    sport: Sport,
    submitted_at: SystemTime,
    returned_at: Option<SystemTime>,
) -> Video {
    Video::restore(
        VideoId::from_u128(id),
        student,
        coach,
        sport,
        submitted_at,
        returned_at.map(|returned_at| {
            (
                VideoReviewId::from_u128(id + 100),
                ReviewComment::new("テイクバックが早い").unwrap(),
                returned_at,
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
    interface::returned_video_reviews_router(
        ListReturnedVideoReviews::new(
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
            // 送付から 48 時間ちょうどで、日本時間の 10-31 の 0 時ちょうどに返した
            video(
                10,
                STUDENT,
                COACH,
                Sport::Tennis,
                oct31_jst() - 48 * HOUR,
                Some(oct31_jst()),
            ),
            // 47 時間 59 分 59 秒で返した（かかった時間は 47）。生徒のアカウントが引けない
            video(
                11,
                UNKNOWN_STUDENT,
                OTHER_COACH,
                Sport::Golf,
                oct31_jst() - 50 * HOUR,
                Some(oct31_jst() - 2 * HOUR - Duration::from_secs(1)),
            ),
            // 日本時間の 10-24 の終わりに返した（既定の 7 日の外）
            video(
                12,
                STUDENT,
                OTHER_COACH,
                Sport::Tennis,
                oct31_jst() - 8 * 24 * HOUR,
                Some(oct31_jst() - 6 * 24 * HOUR - Duration::from_secs(1)),
            ),
            // 返していない動画は載らない
            video(13, STUDENT, COACH, Sport::Tennis, now() - 200 * HOUR, None),
        ]),
    )
}

/// `GET /video-reviews/returned` にクエリ `query`（`?` から。なければ空）を付ける。レスポンスを admin-web の契約で検査する。
async fn list(router: &Router, token: Option<&str>, query: &str) -> (StatusCode, Value) {
    let mut request = Request::get(format!("/video-reviews/returned{query}"));
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
            "/video-reviews/returned",
            status,
            &body,
        );
    }
    (status, body)
}

fn ids(body: &Value) -> Vec<String> {
    body["reviews"]
        .as_array()
        .unwrap()
        .iter()
        .map(|review| review["video_id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn headquarters_lists_returned_video_reviews_of_the_last_seven_days_latest_first() {
    let (status, body) = list(&app().await, Some(HEADQUARTERS), "").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "from": "2026-10-25",
            "to": "2026-10-31",
            "reviews": [
                {
                    "video_id": "00000000-0000-0000-0000-00000000000a",
                    "sport": "tennis",
                    "student_id": "00000000-0000-0000-0000-000000000004",
                    "student_display_name": "あおき",
                    "coach_id": "00000000-0000-0000-0000-000000000002",
                    "coach_display_name": "たなかコーチ",
                    "submitted_at": "2026-10-28T15:00:00Z",
                    "returned_at": "2026-10-30T15:00:00Z",
                    "lead_time_hours": 48,
                },
                {
                    "video_id": "00000000-0000-0000-0000-00000000000b",
                    "sport": "golf",
                    // 生徒のアカウントが引けないので student_display_name は載らない
                    "student_id": "00000000-0000-0000-0000-000000000005",
                    "coach_id": "00000000-0000-0000-0000-000000000003",
                    "coach_display_name": "いとうコーチ",
                    "submitted_at": "2026-10-28T13:00:00Z",
                    "returned_at": "2026-10-30T12:59:59Z",
                    "lead_time_hours": 47,
                },
            ],
        }),
        "既定は今日（日本時間）までの 7 日。期間の外（12）と返していない動画（13）は載らない"
    );
}

#[tokio::test]
async fn the_period_and_coach_narrow_the_list() {
    let router = app().await;
    let (a, b, c) = (
        "00000000-0000-0000-0000-00000000000a",
        "00000000-0000-0000-0000-00000000000b",
        "00000000-0000-0000-0000-00000000000c",
    );
    for (query, from, to, expected) in [
        (
            "?from=2026-10-24&to=2026-10-24",
            "2026-10-24",
            "2026-10-24",
            vec![c],
        ),
        // 返却が 10-31 の日本時間 0 時ちょうど（a）は 10-31 に入り、10-30 には入らない
        (
            "?from=2026-10-30&to=2026-10-30",
            "2026-10-30",
            "2026-10-30",
            vec![b],
        ),
        (
            "?from=2026-10-31&to=2026-10-31",
            "2026-10-31",
            "2026-10-31",
            vec![a],
        ),
        // to だけなら to から遡って 7 日、from だけなら from から 7 日目まで
        ("?to=2026-10-30", "2026-10-24", "2026-10-30", vec![b, c]),
        ("?from=2026-10-20", "2026-10-20", "2026-10-26", vec![c]),
        // 31 日ちょうど
        (
            "?from=2026-10-01&to=2026-10-31",
            "2026-10-01",
            "2026-10-31",
            vec![a, b, c],
        ),
        (
            "?coach_id=00000000-0000-0000-0000-000000000003&from=2026-10-01&to=2026-10-31",
            "2026-10-01",
            "2026-10-31",
            vec![b, c],
        ),
        // いないコーチは空
        (
            "?coach_id=00000000-0000-0000-0000-000000000063",
            "2026-10-25",
            "2026-10-31",
            vec![],
        ),
    ] {
        let (status, body) = list(&router, Some(HEADQUARTERS), query).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(
            (body["from"].as_str(), body["to"].as_str()),
            (Some(from), Some(to)),
            "{query}"
        );
        assert_eq!(ids(&body), expected, "{query}");
    }
}

#[tokio::test]
async fn invalid_query_is_422_validation_failed() {
    let router = app().await;
    for (query, message) in [
        (
            "?from=2026-10-02&to=2026-10-01",
            "to は from と同じ日か、それより後の日付にする",
        ),
        (
            "?from=2026-10-01&to=2026-11-01",
            "from から to までは 31 日まで",
        ),
        ("?from=2026/10/01", "from は YYYY-MM-DD の日付でない"),
        ("?to=2026-02-30", "to は YYYY-MM-DD の日付でない"),
        ("?from=", "from は YYYY-MM-DD の日付でない"),
        ("?coach_id=coach-2", "coach_id が UUID でない"),
    ] {
        let (status, body) = list(&router, Some(HEADQUARTERS), query).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}: {body}");
        assert_eq!(body["code"], "validation_failed", "{query}");
        assert_eq!(body["message"], message, "{query}");
    }
}

#[tokio::test]
async fn no_returned_videos_is_an_empty_list() {
    let router = router(
        InMemoryAccountRepository::default(),
        InMemoryVideoRepository::with_videos([video(
            1,
            STUDENT,
            COACH,
            Sport::Tennis,
            now() - HOUR,
            None,
        )]),
    );
    let (status, body) = list(&router, Some(HEADQUARTERS), "").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "from": "2026-10-25", "to": "2026-10-31", "reviews": [] })
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
