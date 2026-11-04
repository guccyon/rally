//! 生徒がコーチの評価（`getCoachReviewSummary` の件数と平均・`listCoachReviews` の一覧）を読むことを、保存先を Postgres に
//! した組み立て（`server::router_with_postgres`）で通す。評価していない生徒でも、どのコーチの分も読めること、一覧が送った
//! 時刻の新しい順で評価した生徒を載せないこと、評価のないコーチは 0 件になることを確かめる。
//! 評価されたコーチが `PUT /lesson-reviews/{review_id}/reply` で付けた返信（2 回目は上書き）が、生徒の読む一覧に載ることも
//! 本部とコーチ（admin-web）が `max_rating` でその点以下の評価だけを読めること（生徒は付けても変わらないこと）も確かめる。
//! 本部とコーチが `from`・`to` で評価の付いた日付（日本時間）の期間に絞れること（`max_rating` と合わせられること）も確かめる。
//! 契約の形の細かい検査は interface の `tests/reviews.rs`・`tests/review_replies.rs`。
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
    catalog::Sport,
    identity::{AccountId, Role},
    lesson::LessonCompletionId,
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

async fn put(router: &Router, path: &str, token: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::put(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

/// アカウントを登録して ID とトークンを返す（本部以外はテニス）。契約にコーチの登録の操作がないので、ユースケースを直に呼ぶ。
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
        // 本部はスポーツを持たない
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
            LessonCompletionId::from_u128(lesson),
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

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn the_reviewed_coach_replies_and_students_read_the_latest_reply_from_postgres(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (_, ito_token) = register(&pool, "ito@example.com", Role::Coach).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;
    let reviews = PgLessonReviewRepository::new(pool.clone());
    let review_id = review(&reviews, taro, tanaka, 1, 4, "よい", 1).await;
    let router = server::router_with_postgres(pool, SystemClock, RecordingAnalyticsSink::default());
    let path = format!("/lesson-reviews/{review_id}/reply");

    let (status, body) = put(
        &router,
        &path,
        &tanaka_token,
        json!({ "comment": "ありがとう" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["comment"], "ありがとう");
    let (status, body) = put(
        &router,
        &path,
        &tanaka_token,
        json!({ "comment": "次回はサーブも見ます" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let replied_at = body["replied_at"].clone();

    // 他のコーチ・生徒は返信できない
    for token in [&ito_token, &taro_token] {
        let (status, body) = put(&router, &path, token, json!({ "comment": "横取り" })).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }

    let tanaka_id = account_id_uuid::format(tanaka);
    let (status, body) = get(
        &router,
        &format!("/coaches/{tanaka_id}/reviews"),
        &taro_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let reply = &body["reviews"][0]["reply"];
    assert_eq!(
        reply["comment"], "次回はサーブも見ます",
        "最後の返信だけが載る"
    );
    // 応答の時刻は時計の精度、Postgres はマイクロ秒までなので、秒までを比べる
    let seconds = |value: &Value| value.as_str().unwrap()[..19].to_string();
    assert_eq!(seconds(&reply["replied_at"]), seconds(&replied_at));
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_reads_only_reviews_rated_at_most_the_max_rating_from_postgres(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (ito, _) = register(&pool, "ito@example.com", Role::Coach).await;
    let (_, headquarters) = register(&pool, "hq@example.com", Role::Headquarters).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;

    let reviews = PgLessonReviewRepository::new(pool.clone());
    let low_older = review(&reviews, taro, tanaka, 1, 1, "合わなかった", 48).await;
    let high = review(&reviews, taro, tanaka, 2, 5, "とてもよい", 1).await;
    let low_newer = review(&reviews, taro, tanaka, 3, 2, "いまひとつ", 24).await;
    let middle = review(&reviews, taro, tanaka, 4, 3, "ふつう", 12).await;
    // 他のコーチの低い評価は載らない
    review(&reviews, taro, ito, 5, 1, "別のコーチ", 2).await;
    let router = server::router_with_postgres(pool, SystemClock, RecordingAnalyticsSink::default());

    let tanaka_id = account_id_uuid::format(tanaka);
    let ids = |body: &Value| -> Vec<String> {
        body["reviews"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["review_id"].as_str().unwrap().to_string())
            .collect()
    };
    let path = |max_rating: &str| format!("/coaches/{tanaka_id}/reviews?max_rating={max_rating}");

    for token in [&headquarters, &tanaka_token] {
        let (status, body) = get(&router, &path("2"), token).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["coach_id"], tanaka_id);
        assert_eq!(
            ids(&body),
            [low_newer.clone(), low_older.clone()],
            "★2 以下だけを、送った時刻の新しい順"
        );
        let (status, body) = get(&router, &path("3"), token).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            ids(&body),
            [middle.clone(), low_newer.clone(), low_older.clone()]
        );

        for invalid in ["0", "6", "2.5", "low"] {
            let (status, body) = get(&router, &path(invalid), token).await;
            assert_eq!(
                status,
                StatusCode::UNPROCESSABLE_ENTITY,
                "{invalid}: {body}"
            );
            assert_eq!(body["code"], "validation_failed");
        }
    }

    // 生徒（student-app）は付けても変わらない
    let (status, body) = get(&router, &path("2"), &taro_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(ids(&body), [high, middle, low_newer, low_older]);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_reads_only_reviews_submitted_in_the_period_from_postgres(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (ito, _) = register(&pool, "ito@example.com", Role::Coach).await;
    let (_, headquarters) = register(&pool, "hq@example.com", Role::Headquarters).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;

    // NOW は 2026-10-26 9 時（日本時間）
    let reviews = PgLessonReviewRepository::new(pool.clone());
    // 10 月 26 日 8 時と、10 月 26 日の始まり（0 時）
    let today_high = review(&reviews, taro, tanaka, 1, 5, "とてもよい", 1).await;
    let today_low = review(&reviews, taro, tanaka, 2, 2, "いまひとつ", 9).await;
    // 10 月 25 日 23 時
    let yesterday = review(&reviews, taro, tanaka, 3, 1, "合わなかった", 10).await;
    // 10 月 24 日 9 時
    let older = review(&reviews, taro, tanaka, 4, 3, "ふつう", 48).await;
    // 他のコーチの評価は期間に入っても載らない
    review(&reviews, taro, ito, 5, 1, "別のコーチ", 2).await;
    let router = server::router_with_postgres(pool, SystemClock, RecordingAnalyticsSink::default());

    let tanaka_id = account_id_uuid::format(tanaka);
    let ids = |body: &Value| -> Vec<String> {
        body["reviews"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["review_id"].as_str().unwrap().to_string())
            .collect()
    };
    let path = |query: &str| format!("/coaches/{tanaka_id}/reviews?{query}");

    for token in [&headquarters, &tanaka_token] {
        for (query, expected) in [
            (
                "from=2026-10-26&to=2026-10-26",
                vec![today_high.clone(), today_low.clone()],
            ),
            ("from=2026-10-25&to=2026-10-25", vec![yesterday.clone()]),
            (
                "from=2026-10-24&to=2026-10-25",
                vec![yesterday.clone(), older.clone()],
            ),
            (
                "to=2026-10-26",
                vec![
                    today_high.clone(),
                    today_low.clone(),
                    yesterday.clone(),
                    older.clone(),
                ],
            ),
            (
                "from=2026-10-25&to=2026-10-26&max_rating=2",
                vec![today_low.clone(), yesterday.clone()],
            ),
        ] {
            let (status, body) = get(&router, &path(query), token).await;
            assert_eq!(status, StatusCode::OK, "{query}: {body}");
            assert_eq!(body["coach_id"], tanaka_id);
            assert_eq!(ids(&body), expected, "{query}");
        }

        let (status, body) = get(&router, &path("from=2026-10-26&to=2026-10-25"), token).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(body["code"], "validation_failed");
    }

    // 生徒（student-app）は付けても変わらない
    let (status, body) = get(&router, &path("from=2026-10-26&to=2026-10-26"), &taro_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(ids(&body), [today_high, today_low, yesterday, older]);
}
