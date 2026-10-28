//! 生徒がコーチの評価（`getCoachReviewSummary` の件数と平均・`listCoachReviews` の一覧）を読むことを、保存先を Postgres に
//! した組み立て（`server::router_with_postgres`）で通す。評価していない生徒でも、どのコーチの分も読めること、一覧が送った
//! 時刻の新しい順で評価した生徒を載せないこと、評価のないコーチは 0 件になることを確かめる。
//! 契約の形の細かい検査は interface の `tests/reviews.rs`。
//!
//! 評価は完了したレッスンが要り、そこまでの流れは `lessons.rs` で通しているので、ここでは評価を保存先に直に書く
//! （`review.lesson_reviews` の `lesson_id` には外部キーがない）。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test coach_reviews -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    clock::SystemClock,
    identity::{
        RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer, account_id_uuid,
    },
    in_memory::RecordingAnalyticsSink,
    review::{LessonReviewRepository, lesson_review_id_uuid},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    identity::{AccountId, Role, Sport},
    lesson::LessonId,
    review::{LessonReview, LessonReviewComment, Rating},
};
use infra::{MockTokenService, PgAccountRepository, PgLessonReviewRepository};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

/// 2026-10-26T00:00:00Z
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
        sports: vec![Sport::Tennis],
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

/// 生徒 `student` がコーチ `coach` に付けた評価を保存先に書き、評価の ID を返す。
async fn review(
    reviews: &PgLessonReviewRepository,
    student: AccountId,
    coach: AccountId,
    lesson: u128,
    rating: i64,
    comment: &str,
    hours_ago: u32,
) -> String {
    let id = reviews.next_id();
    reviews
        .insert(LessonReview::restore(
            id,
            LessonId::from_u128(lesson),
            student,
            coach,
            Rating::new(rating).unwrap(),
            LessonReviewComment::new(comment).unwrap(),
            SystemTime::UNIX_EPOCH + NOW - HOUR * hours_ago,
        ))
        .await
        .unwrap();
    lesson_review_id_uuid::format(id)
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_student_reads_the_reviews_of_any_coach_from_postgres(pool: PgPool) {
    let (tanaka, _) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (ito, _) = register(&pool, "ito@example.com", Role::Coach).await;
    let (taro, _) = register(&pool, "taro@example.com", Role::Student).await;
    let (jiro, _) = register(&pool, "jiro@example.com", Role::Student).await;
    // 評価をまだ付けていない生徒が読む
    let (_, hanako) = register(&pool, "hanako@example.com", Role::Student).await;

    let reviews = PgLessonReviewRepository::new(pool.clone());
    let older = review(&reviews, taro, tanaka, 1, 3, "ふつう", 48).await;
    let newer = review(&reviews, jiro, tanaka, 2, 4, "よい", 1).await;
    let middle = review(&reviews, taro, tanaka, 3, 4, "またお願いします", 24).await;
    let router = server::router_with_postgres(pool, SystemClock, RecordingAnalyticsSink::default());

    let tanaka_id = account_id_uuid::format(tanaka);
    let (status, body) = get(
        &router,
        &format!("/coaches/{tanaka_id}/review-summary"),
        &hanako,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // (3 + 4 + 4) / 3 = 3.666… を小数第 2 位まで
    assert_eq!(
        body,
        json!({ "coach_id": tanaka_id, "review_count": 3, "average_rating": 3.67 })
    );

    let (status, body) = get(&router, &format!("/coaches/{tanaka_id}/reviews"), &hanako).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["coach_id"], tanaka_id);
    let items = body["reviews"].as_array().unwrap();
    let ids: Vec<&str> = items
        .iter()
        .map(|item| item["review_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [&newer, &middle, &older], "送った時刻の新しい順");
    assert_eq!(items[0]["comment"], "よい");
    assert_eq!(items[0]["submitted_at"], "2026-10-25T23:00:00Z");
    for item in items {
        assert!(
            item.get("student_id").is_none(),
            "評価した生徒は載せない: {item}"
        );
    }

    // 評価のないコーチは 0 件
    let ito_id = account_id_uuid::format(ito);
    let (status, body) = get(
        &router,
        &format!("/coaches/{ito_id}/review-summary"),
        &hanako,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "coach_id": ito_id, "review_count": 0, "average_rating": null })
    );
    let (status, body) = get(&router, &format!("/coaches/{ito_id}/reviews"), &hanako).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "coach_id": ito_id, "reviews": [] }));
}
