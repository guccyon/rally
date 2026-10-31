//! `/coaches/me/lesson-stats` の統合テスト。admin-web の `GET /coaches/me/lesson-stats`（`getMyLessonStats`）の
//! レスポンスの形を OpenAPI のスキーマで検査し、トークンのコーチ自身の今月と先月（日本時間の暦月）の完了したレッスンの数
//! （同期・非同期の別）と評価の件数と平均を返すこと・月の境目（日本時間の 1 日 0 時）・0 件の月・コーチ以外（生徒・本部）は
//! 403、トークンがなければ 401 になることを確かめる。

use std::time::{Duration, SystemTime};

use application::{
    identity::{
        AccessToken, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier, account_id_uuid,
    },
    in_memory::{FixedClock, InMemoryLessonCompletionRepository, InMemoryLessonReviewRepository},
    lesson::GetMyLessonStats,
    review::ReviewMonthlyReviewSummaryLookup,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    async_lesson::VideoId,
    identity::{AccountId, Role},
    lesson::{LessonCompletion, LessonCompletionId, LessonSource},
    live_lesson::SessionId,
    review::{LessonReview, LessonReviewComment, LessonReviewId, Rating},
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

const COACH: &str = "coach:9";
const OTHER_COACH: &str = "coach:8";
const NEW_COACH: &str = "coach:7";
const STUDENT: &str = "student:1";
const HEADQUARTERS: &str = "headquarters:5";

fn account_id(token: &str) -> AccountId {
    AccountId::from_u128(token.split_once(':').unwrap().1.parse().unwrap())
}

/// 2026-11-01T00:00:00+09:00（2026-10-31T15:00:00Z）
fn nov1() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_458_800)
}

/// 2026-10-01T00:00:00+09:00（2026-09-30T15:00:00Z）
fn oct1() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_780_400)
}

const SECOND: Duration = Duration::from_secs(1);

fn completion(id: u128, coach: &str, live: bool, at: SystemTime) -> LessonCompletion {
    let source = if live {
        LessonSource::Live(SessionId::from_u128(id))
    } else {
        LessonSource::Async(VideoId::from_u128(id))
    };
    LessonCompletion::restore(
        LessonCompletionId::from_u128(id),
        AccountId::from_u128(100 + id),
        account_id(coach),
        source,
        at,
        false,
    )
}

fn review(id: u128, coach: &str, rating: i64, at: SystemTime) -> LessonReview {
    LessonReview::restore(
        LessonReviewId::from_u128(id),
        LessonCompletionId::from_u128(id),
        AccountId::from_u128(100 + id),
        account_id(coach),
        Rating::new(rating).unwrap(),
        LessonReviewComment::new("よい").unwrap(),
        at,
    )
}

/// 今（`now`）に呼ぶルーター。コーチ `COACH` は 11 月（日本時間）に通話 1・添削 2、10 月に通話 2（10 月 1 日 0 時ちょうどと
/// 10 月 31 日 23:59:59）、9 月に通話 1 を完了し、評価は 11 月に 5、10 月に 4 と 3、9 月に 1 を受けた。
fn app(now: SystemTime) -> Router {
    let completions = InMemoryLessonCompletionRepository::with_completions([
        completion(1, COACH, true, nov1()),
        completion(2, COACH, false, nov1() + SECOND),
        completion(3, COACH, false, nov1() + Duration::from_secs(3600)),
        completion(4, COACH, true, nov1() - SECOND),
        completion(5, COACH, true, oct1()),
        completion(6, COACH, true, oct1() - SECOND),
        completion(7, OTHER_COACH, true, nov1()),
    ]);
    let reviews = InMemoryLessonReviewRepository::with_reviews([
        review(1, COACH, 5, nov1()),
        review(4, COACH, 4, nov1() - SECOND),
        review(5, COACH, 3, oct1()),
        review(6, COACH, 1, oct1() - SECOND),
        review(7, OTHER_COACH, 2, nov1()),
    ]);
    interface::lesson_stats_router(
        GetMyLessonStats::new(
            completions,
            ReviewMonthlyReviewSummaryLookup::new(reviews),
            FixedClock(now),
        ),
        FakeTokens,
    )
}

async fn get_stats(router: &Router, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get("/coaches/me/lesson-stats");
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
        "/coaches/me/lesson-stats",
        status,
        &body,
    );
    (status, body)
}

fn month(
    month: &str,
    (live, async_lesson): (u32, u32),
    review_count: u32,
    average: Value,
) -> Value {
    json!({
        "month": month,
        "live_lesson_count": live,
        "async_lesson_count": async_lesson,
        "review_count": review_count,
        "average_rating": average,
    })
}

#[tokio::test]
async fn a_coach_reads_this_month_and_last_month_split_at_midnight_in_japan() {
    let router = app(nov1() + Duration::from_secs(2 * 3600));

    let (status, body) = get_stats(&router, Some(COACH)).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "coach_id": account_id_uuid::format(account_id(COACH)),
            "current_month": month("2026-11", (1, 2), 1, json!(5.0)),
            "previous_month": month("2026-10", (2, 0), 2, json!(3.5)),
        }),
        "日本時間の 11 月 1 日 0 時から今月。9 月の分と他のコーチの分は数えない"
    );
}

#[tokio::test]
async fn one_second_before_midnight_in_japan_this_month_is_still_october() {
    // 日本時間の 10 月 31 日 23:59:59（UTC では 10 月 31 日 14:59:59）
    let router = app(nov1() - SECOND);

    let (status, body) = get_stats(&router, Some(COACH)).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["current_month"],
        month("2026-10", (2, 0), 2, json!(3.5))
    );
    assert_eq!(
        body["previous_month"],
        month("2026-09", (1, 0), 1, json!(1.0))
    );
}

#[tokio::test]
async fn a_coach_without_lessons_gets_zero_and_a_null_average() {
    let router = app(nov1());

    let (status, body) = get_stats(&router, Some(NEW_COACH)).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "coach_id": account_id_uuid::format(account_id(NEW_COACH)),
            "current_month": month("2026-11", (0, 0), 0, Value::Null),
            "previous_month": month("2026-10", (0, 0), 0, Value::Null),
        })
    );
}

#[tokio::test]
async fn only_coaches_can_read_lesson_stats() {
    let router = app(nov1());
    for token in [STUDENT, HEADQUARTERS] {
        let (status, body) = get_stats(&router, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert!(body.get("current_month").is_none(), "{token}: {body}");
    }
    let (status, _) = get_stats(&router, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
