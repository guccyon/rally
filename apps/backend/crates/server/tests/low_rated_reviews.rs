//! 本部が見る低い評価の一覧（admin-web の `GET /lesson-reviews/low`）を、保存先を Postgres にした組み立て
//! （`server::router_with_postgres`）で通す。直近 30 日の★2 以下の評価だけがコーチを問わず新しい順に並び、コーチ・生徒の
//! 表示名がアカウントの表から添わること、コーチ・生徒は 403 であることを確かめる。契約の形の細かい検査は interface の
//! `tests/low_rated_reviews.rs`。
//!
//! 評価は完了したレッスンが要り、そこまでの流れは `lessons.rs` で通しているので、ここでは評価を保存先に直に書く
//! （`review.lesson_reviews` の `lesson_id` には外部キーがない）。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test low_rated_reviews -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    identity::{
        RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer, account_id_uuid,
    },
    in_memory::{FixedClock, RecordingAnalyticsSink},
    review::{LessonReviewRepository, lesson_review_id_uuid},
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
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

/// 照会する時刻（2026-10-31T00:00:00Z）
const NOW: Duration = Duration::from_secs(1_793_404_800);
const DAY: Duration = Duration::from_secs(24 * 3600);

fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + NOW
}

async fn get(router: &Router, token: &str) -> (StatusCode, Value) {
    let request = Request::get("/lesson-reviews/low")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

/// アカウントを登録して ID とトークンを返す（表示名はメールアドレスの `@` の前）。契約にコーチ・本部の登録の操作が
/// ないので、ユースケースを直に呼ぶ。
async fn register(pool: &PgPool, email: &str, role: Role) -> (AccountId, String) {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        FixedClock(now() - 60 * DAY),
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

/// 生徒 `student` がコーチ `coach` に `submitted_at` に付けた評価を保存先に書き、評価の ID を返す。
async fn review(
    reviews: &PgLessonReviewRepository,
    student: AccountId,
    coach: AccountId,
    lesson: u128,
    rating: i64,
    submitted_at: SystemTime,
) -> String {
    let id = reviews.next_id();
    reviews
        .insert(LessonReview::restore(
            id,
            LessonCompletionId::from_u128(lesson),
            student,
            coach,
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("説明が少なかった").unwrap(),
            submitted_at,
        ))
        .await
        .unwrap();
    lesson_review_id_uuid::format(id)
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_reads_low_reviews_across_coaches_from_postgres(pool: PgPool) {
    let (_, headquarters) = register(&pool, "honbu@example.com", Role::Headquarters).await;
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (ito, _) = register(&pool, "ito@example.com", Role::Coach).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;

    let reviews = PgLessonReviewRepository::new(pool.clone());
    let older = review(&reviews, taro, tanaka, 1, 1, now() - 20 * DAY).await;
    let newer = review(&reviews, taro, ito, 2, 2, now() - DAY).await;
    review(&reviews, taro, tanaka, 3, 3, now()).await;
    review(&reviews, taro, ito, 4, 1, now() - 31 * DAY).await;
    let router =
        server::router_with_postgres(pool, FixedClock(now()), RecordingAnalyticsSink::default());

    let (status, body) = get(&router, &headquarters).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (body["max_rating"].clone(), body["period_days"].clone()),
        (2.into(), 30.into())
    );
    let items = body["reviews"].as_array().unwrap();
    let ids: Vec<&str> = items
        .iter()
        .map(|item| item["review_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [&newer, &older], "★3 と 30 日より前は載らず、新しい順");
    assert_eq!(items[0]["coach_id"], account_id_uuid::format(ito));
    assert_eq!(items[0]["coach_display_name"], "ito");
    assert_eq!(items[0]["student_id"], account_id_uuid::format(taro));
    assert_eq!(items[0]["student_display_name"], "taro");
    assert_eq!(items[1]["coach_display_name"], "tanaka");
    assert_eq!(items[1]["lesson_menu"], Value::Null);

    for token in [&tanaka_token, &taro_token] {
        let (status, body) = get(&router, token).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
}
