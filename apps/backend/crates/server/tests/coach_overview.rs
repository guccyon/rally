//! 本部がコーチ 1 人の様子（admin-web の `getCoachOverview`）を読むことを、保存先を Postgres にした組み立て
//! （`server::router_with_postgres`）で通す。ルートが組み立てに入っていること、受付の状態・評価の件数と平均・直近の低い評価の
//! 件数と未対応の件数（本部が `PUT /lesson-reviews/{review_id}/handling` で付けた印を読む）・今月と先月の実績・公開中の
//! レッスンメニューの数を、それぞれの保存先（scheduling・review・lesson・catalog）から読むこと、照会のあとも何も変わらないこと、ないコーチは 404・本部以外は 403 になることを確かめる。契約の形と数え方の細かい検査は interface の
//! `tests/coach_overviews.rs` と各操作のテスト。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test coach_overview -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    clock::SystemClock,
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
    review::{LessonReview, LessonReviewComment, LessonReviewId, Rating},
};
use infra::{MockTokenService, PgAccountRepository, PgLessonReviewRepository};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

/// 2026-10-26T00:00:00Z（日本時間の 10 月 26 日 9 時）
const NOW: Duration = Duration::from_secs(1_792_972_800);
const HOUR: Duration = Duration::from_secs(3600);

async fn send(
    router: &Router,
    method: &str,
    path: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json");
    let body = body.map_or_else(Body::empty, |b| Body::from(b.to_string()));
    let response = router
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

/// アカウントを登録して ID とトークンを返す。契約にコーチ・本部の登録の操作がないので、ユースケースを直に呼ぶ。
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

/// 生徒 `student` がコーチ `coach` に `submitted_at` に付けた評価を保存先に書き、評価の ID を返す（`lesson_id` には外部キーが
/// ない）。
async fn review(
    reviews: &PgLessonReviewRepository,
    student: AccountId,
    coach: AccountId,
    lesson: u128,
    rating: i64,
    submitted_at: SystemTime,
) -> LessonReviewId {
    let review_id = reviews.next_id();
    reviews
        .insert(LessonReview::restore(
            review_id,
            LessonCompletionId::from_u128(lesson),
            student,
            coach,
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("よい").unwrap(),
            submitted_at,
        ))
        .await
        .unwrap();
    review_id
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_reads_the_overview_of_a_coach_from_postgres(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (ito, _) = register(&pool, "ito@example.com", Role::Coach).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;
    let (_, hq_token) = register(&pool, "hq@example.com", Role::Headquarters).await;

    let now = SystemTime::UNIX_EPOCH + NOW;
    let reviews = PgLessonReviewRepository::new(pool.clone());
    review(&reviews, taro, tanaka, 1, 4, now - HOUR).await;
    // 2026-09-30T00:00:00Z（日本時間の 9 月 30 日 9 時）は先月
    review(&reviews, taro, tanaka, 2, 5, now - HOUR * 24 * 26).await;
    review(&reviews, taro, ito, 3, 1, now - HOUR).await;
    // 直近 30 日の低い評価は★2 と★1 の 2 件（★2 には本部があとで対応済みの印を付ける）。31 日前の★1 は数えない
    let handled = review(&reviews, taro, tanaka, 4, 2, now - HOUR * 2).await;
    review(&reviews, taro, tanaka, 5, 1, now - HOUR * 3).await;
    review(&reviews, taro, tanaka, 6, 1, now - HOUR * 24 * 31).await;
    let router =
        server::router_with_postgres(pool, FixedClock(now), RecordingAnalyticsSink::default());

    // メニューは公開中 2（うち 1 つを非公開に直す）と、ほかに公開中 1
    let mut menu_ids = Vec::new();
    for name in ["フォームの添削", "サーブの添削", "ボレーの添削"] {
        let (status, menu) = send(
            &router,
            "POST",
            "/coaches/me/lesson-menus",
            &tanaka_token,
            Some(json!({ "sport": "tennis", "lesson_type": "async_lesson", "name": name })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{menu}");
        menu_ids.push(menu["lesson_menu_id"].as_str().unwrap().to_string());
    }
    let (status, body) = send(
        &router,
        "PUT",
        &format!("/coaches/me/lesson-menus/{}", menu_ids[2]),
        &tanaka_token,
        Some(json!({
            "sport": "tennis",
            "lesson_type": "async_lesson",
            "name": "ボレーの添削",
            "description": "",
            "is_published": false
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tanaka_id = account_id_uuid::format(tanaka);
    let (status, body) = send(
        &router,
        "POST",
        &format!("/coaches/{tanaka_id}/suspend"),
        &hq_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let path = format!("/coaches/{tanaka_id}/overview");
    let (status, body) = send(&router, "GET", &path, &hq_token, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            &body["low_rated_review_count"],
            &body["unhandled_low_rated_review_count"]
        ),
        (&json!(2), &json!(2)),
        "印を付ける前は 2 件とも未対応"
    );
    let (status, body) = send(
        &router,
        "PUT",
        &format!(
            "/lesson-reviews/{}/handling",
            lesson_review_id_uuid::format(handled)
        ),
        &hq_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = send(&router, "GET", &path, &hq_token, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let expected = json!({
        "coach_id": tanaka_id,
        "display_name": "tanaka",
        "acceptance_status": "suspended",
        "review_count": 5,
        "average_rating": 2.6,
        "low_rated_review_count": 2,
        "unhandled_low_rated_review_count": 1,
        "current_month": {
            "month": "2026-10",
            "live_lesson_count": 0,
            "async_lesson_count": 0,
            "review_count": 3,
            "average_rating": 2.33
        },
        "previous_month": {
            "month": "2026-09",
            "live_lesson_count": 0,
            "async_lesson_count": 0,
            "review_count": 2,
            "average_rating": 3.0
        },
        "published_lesson_menu_count": 2
    });
    assert_eq!(body, expected);
    // 照会だけで何も変えない（2 回目も同じ）
    let (_, again) = send(&router, "GET", &path, &hq_token, None).await;
    assert_eq!(again, expected);

    let (status, body) = send(
        &router,
        "GET",
        &format!("/coaches/{}/overview", account_id_uuid::format(ito)),
        &hq_token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["acceptance_status"], "accepting");
    assert_eq!(body["review_count"], 1);
    assert_eq!(body["low_rated_review_count"], 1);
    assert_eq!(body["unhandled_low_rated_review_count"], 1);
    assert_eq!(body["published_lesson_menu_count"], 0);

    let student_path = format!("/coaches/{}/overview", account_id_uuid::format(taro));
    let (status, _) = send(&router, "GET", &student_path, &hq_token, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    for token in [&tanaka_token, &taro_token] {
        let (status, _) = send(&router, "GET", &path, token, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
}
