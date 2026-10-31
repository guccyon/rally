//! コーチが自分の今月と先月のレッスンの実績（admin-web の `getMyLessonStats`）を読むことを、保存先を Postgres にした
//! 組み立て（`server::router_with_postgres`）で通す。ルートが組み立てに入っていること、評価の件数と平均を review の保存先から
//! 日本時間の暦月ごとに読むこと、レッスンのないコーチは 0 件になること、生徒・本部は 403 になることを確かめる。
//! 月の境目と契約の形の細かい検査は interface の `tests/lesson_stats.rs`、完了したレッスンのコーチごとの読み取りは
//! infra の `tests/pg_lesson_completion_repository.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test lesson_stats -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    clock::SystemClock,
    identity::{
        RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer, account_id_uuid,
    },
    in_memory::{FixedClock, RecordingAnalyticsSink},
    review::LessonReviewRepository,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    identity::{AccountId, Role},
    lesson::LessonCompletionId,
    review::{LessonReview, LessonReviewComment, Rating},
};
use infra::{MockTokenService, PgAccountRepository, PgLessonReviewRepository};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

/// 2026-10-26T00:00:00Z（日本時間の 10 月 26 日 9 時）
const NOW: Duration = Duration::from_secs(1_792_972_800);
const HOUR: Duration = Duration::from_secs(3600);

async fn get(router: &Router, path: &str, token: &str) -> (StatusCode, Value) {
    let request = Request::get(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

/// アカウントを登録して ID とトークンを返す。契約にコーチの登録の操作がないので、ユースケースを直に呼ぶ。
async fn register(pool: &PgPool, email: &str, role: Role) -> (AccountId, String) {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        SystemClock,
    )
    .execute(RegisterAccountCommand {
        email: email.to_string(),
        display_name: email.split('@').next().unwrap().to_string(),
        role,
        sports: if role == Role::Headquarters {
            vec![]
        } else {
            vec![Sport::Tennis]
        },
    })
    .await
    .unwrap();
    let token = MockTokenService
        .issue(TokenClaims {
            account_id: account.id(),
            role,
        })
        .into_string();
    (account.id(), token)
}

/// 生徒 `student` がコーチ `coach` に `submitted_at` に付けた評価を保存先に書く（`lesson_id` には外部キーがない）。
async fn review(
    reviews: &PgLessonReviewRepository,
    student: AccountId,
    coach: AccountId,
    lesson: u128,
    rating: i64,
    submitted_at: SystemTime,
) {
    reviews
        .insert(LessonReview::restore(
            reviews.next_id(),
            LessonCompletionId::from_u128(lesson),
            student,
            coach,
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("よい").unwrap(),
            submitted_at,
        ))
        .await
        .unwrap();
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_coach_reads_own_lesson_stats_from_postgres(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (ito, ito_token) = register(&pool, "ito@example.com", Role::Coach).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;
    let (_, hq_token) = register(&pool, "hq@example.com", Role::Headquarters).await;

    let now = SystemTime::UNIX_EPOCH + NOW;
    let reviews = PgLessonReviewRepository::new(pool.clone());
    review(&reviews, taro, tanaka, 1, 4, now - HOUR).await;
    review(&reviews, taro, tanaka, 2, 5, now - HOUR * 2).await;
    // 2026-09-30T00:00:00Z（日本時間の 9 月 30 日 9 時）は先月
    review(&reviews, taro, tanaka, 3, 2, now - HOUR * 24 * 26).await;
    let router =
        server::router_with_postgres(pool, FixedClock(now), RecordingAnalyticsSink::default());

    let (status, body) = get(&router, "/coaches/me/lesson-stats", &tanaka_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "coach_id": account_id_uuid::format(tanaka),
            "current_month": {
                "month": "2026-10",
                "live_lesson_count": 0,
                "async_lesson_count": 0,
                "review_count": 2,
                "average_rating": 4.5
            },
            "previous_month": {
                "month": "2026-09",
                "live_lesson_count": 0,
                "async_lesson_count": 0,
                "review_count": 1,
                "average_rating": 2.0
            }
        })
    );

    let (status, body) = get(&router, "/coaches/me/lesson-stats", &ito_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["coach_id"], account_id_uuid::format(ito));
    assert_eq!(body["current_month"]["review_count"], 0);
    assert_eq!(body["current_month"]["average_rating"], Value::Null);

    for token in [&taro_token, &hq_token] {
        let (status, _) = get(&router, "/coaches/me/lesson-stats", token).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
}
