//! `/coaches/{coach_id}/overview` の統合テスト。admin-web の `GET /coaches/{coach_id}/overview`（`getCoachOverview`）の
//! レスポンスの形を OpenAPI のスキーマで検査し、本部がコーチ 1 人の表示名・受付の状態・評価の件数と平均・今月と先月
//! （日本時間の暦月）のレッスンの実績・公開中のレッスンメニューの数・直近 30 日の動画添削の件数と返却までの時間の中央値を
//! 1 回で読めること、コーチでない ID・ない ID・UUID でない
//! ID は 404、本部以外（コーチ・生徒）は 403、トークンがなければ 401 になることを確かめる。数え方の細かい検査は各操作の
//! テスト（`reviews.rs`・`lesson_stats.rs`・`lesson_menus.rs`）と、domain の `VideoReviewLeadTime` の単体テスト。

use std::time::{Duration, SystemTime};

use application::{
    async_lesson::GetCoachVideoReviewLeadTime,
    catalog::ListCoachLessonMenus,
    identity::{
        AccessToken, AccountRepository, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier,
        account_id_uuid,
    },
    in_memory::{
        FixedClock, InMemoryAccountRepository, InMemoryCoachAcceptanceRepository,
        InMemoryLessonCompletionRepository, InMemoryLessonMenuRepository,
        InMemoryLessonReviewRepository, InMemoryVideoRepository,
    },
    lesson::GetMyLessonStats,
    profile::GetCoachOverview,
    review::{GetCoachReviewSummary, ReviewMonthlyReviewSummaryLookup},
    scheduling::{CoachAcceptanceRepository, SchedulingCoachAcceptanceLookup},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    async_lesson::{ReviewComment, Video, VideoId, VideoReviewId},
    catalog::{
        LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription, LessonMenuId,
        LessonMenuName, Sport,
    },
    identity::{Account, AccountId, Email, Role},
    lesson::{LessonCompletion, LessonCompletionId, LessonSource},
    live_lesson::SessionId,
    profile::DisplayName,
    review::{LessonReview, LessonReviewComment, LessonReviewId, Rating},
};
use serde_json::{Value, json};
use tower::ServiceExt;

mod support;

/// 受付を止める・再開する本部のアカウント。
const HEADQUARTERS_ID: AccountId = AccountId::from_u128(900);

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

const HEADQUARTERS: &str = "headquarters:5";
const COACH_TOKEN: &str = "coach:9";
const STUDENT_TOKEN: &str = "student:1";

const COACH: AccountId = AccountId::from_u128(9);
const NEW_COACH: AccountId = AccountId::from_u128(8);
const STUDENT: AccountId = AccountId::from_u128(1);

/// 2026-11-01T00:00:00+09:00（2026-10-31T15:00:00Z）
fn nov1() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_458_800)
}

/// 2026-10-01T00:00:00+09:00（2026-09-30T15:00:00Z）
fn oct1() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_780_400)
}

const SECOND: Duration = Duration::from_secs(1);

fn account(id: AccountId, display_name: &str, role: Role) -> Account {
    let sports = if role == Role::Headquarters {
        vec![]
    } else {
        vec![Sport::Tennis]
    };
    Account::new(
        id,
        Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
        DisplayName::new(display_name).unwrap(),
        role,
        sports,
        SystemTime::UNIX_EPOCH,
    )
    .unwrap()
}

fn completion(id: u128, live: bool, at: SystemTime) -> LessonCompletion {
    let source = if live {
        LessonSource::Live(SessionId::from_u128(id))
    } else {
        LessonSource::Async(VideoId::from_u128(id))
    };
    LessonCompletion::restore(
        LessonCompletionId::from_u128(id),
        STUDENT,
        COACH,
        source,
        at,
        false,
    )
}

fn review(id: u128, rating: i64, at: SystemTime) -> LessonReview {
    LessonReview::restore(
        LessonReviewId::from_u128(id),
        LessonCompletionId::from_u128(id),
        STUDENT,
        COACH,
        Rating::new(rating).unwrap(),
        LessonReviewComment::new("よい").unwrap(),
        at,
    )
}

fn menu(id: u128, sport: Sport, is_published: bool) -> LessonMenu {
    LessonMenu::restore(
        LessonMenuId::from_u128(id),
        COACH,
        LessonMenuContent {
            sport,
            format: LessonFormat::Async,
            name: LessonMenuName::new("添削").unwrap(),
            description: LessonMenuDescription::default(),
        },
        is_published,
        SystemTime::UNIX_EPOCH,
    )
}

/// コーチ `COACH` に `returned_at` の `lead_minutes` 分前に送られ、`returned_at` に添削を返した動画。
fn returned_video(id: u128, returned_at: SystemTime, lead_minutes: u64) -> Video {
    Video::restore(
        VideoId::from_u128(id),
        STUDENT,
        COACH,
        Sport::Tennis,
        returned_at - Duration::from_secs(lead_minutes * 60),
        Some((
            VideoReviewId::from_u128(id),
            ReviewComment::new("よい").unwrap(),
            returned_at,
        )),
        None,
    )
    .unwrap()
}

/// 今は日本時間の 11 月 1 日 2 時。コーチ `COACH`（たなか）は受付を止められていて、11 月に通話 1・添削 1、10 月に通話 1 を
/// 完了し、評価は 5（11 月）と 4（10 月）と 2（9 月）を受け、公開中のメニューはテニス 1・ゴルフ 1（ほかに非公開 1）。
/// 動画添削は直近 30 日に 2 本（送付から 1 時間 30 分と 2 時間 10 分）を返し、40 日前に 1 本を返し、1 本はまだ返していない。
/// `NEW_COACH`（いとう）は何もない。
async fn app() -> Router {
    let accounts = InMemoryAccountRepository::default();
    for account in [
        account(COACH, "たなか", Role::Coach),
        account(NEW_COACH, "いとう", Role::Coach),
        account(STUDENT, "あおき", Role::Student),
        account(AccountId::from_u128(5), "うえの", Role::Headquarters),
    ] {
        accounts.insert(account).await.unwrap();
    }
    let acceptances = InMemoryCoachAcceptanceRepository::default();
    acceptances
        .update(COACH, |a| {
            a.suspend(HEADQUARTERS_ID, SystemTime::UNIX_EPOCH)
        })
        .await
        .unwrap();
    let now = nov1() + Duration::from_secs(2 * 3600);
    let day = Duration::from_secs(24 * 3600);
    let reviews = InMemoryLessonReviewRepository::with_reviews([
        review(1, 5, nov1()),
        review(2, 4, nov1() - SECOND),
        review(3, 2, oct1() - SECOND),
    ]);
    interface::coach_overview_router(
        GetCoachOverview::new(
            accounts,
            SchedulingCoachAcceptanceLookup::new(acceptances),
            GetCoachReviewSummary::new(reviews.clone()),
            GetMyLessonStats::new(
                InMemoryLessonCompletionRepository::with_completions([
                    completion(1, true, nov1()),
                    completion(2, false, nov1() + SECOND),
                    completion(3, true, nov1() - SECOND),
                ]),
                ReviewMonthlyReviewSummaryLookup::new(reviews),
                FixedClock(now),
            ),
            ListCoachLessonMenus::new(InMemoryLessonMenuRepository::with_menus([
                menu(1, Sport::Tennis, true),
                menu(2, Sport::Golf, true),
                menu(3, Sport::Tennis, false),
            ])),
            GetCoachVideoReviewLeadTime::new(
                InMemoryVideoRepository::with_videos([
                    returned_video(1, now - day, 90),
                    returned_video(2, now - day * 29, 130),
                    returned_video(3, now - day * 40, 10),
                    Video::submit(
                        VideoId::from_u128(4),
                        STUDENT,
                        COACH,
                        Sport::Tennis,
                        now - day,
                    ),
                ]),
                FixedClock(now),
            ),
        ),
        FakeTokens,
    )
}

async fn get_overview(router: &Router, coach_id: &str, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get(format!("/coaches/{coach_id}/overview"));
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
        "/coaches/{coach_id}/overview",
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
async fn headquarters_reads_the_overview_of_a_coach() {
    let router = app().await;
    let coach_id = account_id_uuid::format(COACH);

    let (status, body) = get_overview(&router, &coach_id, Some(HEADQUARTERS)).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "coach_id": coach_id,
            "display_name": "たなか",
            "acceptance_status": "suspended",
            "review_count": 3,
            "average_rating": 3.67,
            "current_month": month("2026-11", (1, 1), 1, json!(5.0)),
            "previous_month": month("2026-10", (1, 0), 1, json!(4.0)),
            "published_lesson_menu_count": 2,
            "returned_video_review_count": 2,
            "median_video_review_lead_time_hours": 1.83,
        })
    );
}

#[tokio::test]
async fn a_coach_without_anything_is_accepting_with_zero_counts() {
    let router = app().await;
    let coach_id = account_id_uuid::format(NEW_COACH);

    let (status, body) = get_overview(&router, &coach_id, Some(HEADQUARTERS)).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "coach_id": coach_id,
            "display_name": "いとう",
            "acceptance_status": "accepting",
            "review_count": 0,
            "average_rating": null,
            "current_month": month("2026-11", (0, 0), 0, Value::Null),
            "previous_month": month("2026-10", (0, 0), 0, Value::Null),
            "published_lesson_menu_count": 0,
            "returned_video_review_count": 0,
            "median_video_review_lead_time_hours": null,
        })
    );
}

#[tokio::test]
async fn not_a_coach_is_not_found() {
    let router = app().await;
    for coach_id in [
        account_id_uuid::format(STUDENT),
        account_id_uuid::format(AccountId::from_u128(5)),
        account_id_uuid::format(AccountId::from_u128(404)),
        "not-a-uuid".to_string(),
    ] {
        let (status, body) = get_overview(&router, &coach_id, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{coach_id}: {body}");
        assert_eq!(body["code"], "not_found", "{coach_id}: {body}");
    }
}

#[tokio::test]
async fn only_headquarters_can_read_the_overview() {
    let router = app().await;
    let coach_id = account_id_uuid::format(COACH);
    for token in [COACH_TOKEN, STUDENT_TOKEN] {
        let (status, body) = get_overview(&router, &coach_id, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
    }
    let (status, _) = get_overview(&router, &coach_id, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
