//! 本部がコーチ 1 人の様子（admin-web の `getCoachOverview`）を読むことを、保存先を Postgres にした組み立て
//! （`server::router_with_postgres`）で通す。ルートが組み立てに入っていること、受付の状態・評価の件数と平均・今月と先月の
//! 実績・公開中のレッスンメニューの数・直近 30 日の動画添削の件数と返却までの時間の中央値を、それぞれの保存先
//! （scheduling・review・lesson・catalog・async-lesson）から読むこと、照会のあとも
//! 何も変わらないこと、ないコーチは 404・本部以外は 403 になることを確かめる。契約の形と数え方の細かい検査は interface の
//! `tests/coach_overviews.rs` と各操作のテスト。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test coach_overview -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    async_lesson::VideoRepository,
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
    async_lesson::{ReviewComment, Video},
    catalog::Sport,
    identity::{AccountId, Role},
    lesson::LessonCompletionId,
    review::{LessonReview, LessonReviewComment, Rating},
};
use infra::{MockTokenService, PgAccountRepository, PgLessonReviewRepository, PgVideoRepository};
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

/// 生徒 `student` が `submitted_at` にコーチ `coach` に送った動画を保存先に書き、`returned_at` が `Some` ならその時刻に
/// コーチが添削を返したことにする。
async fn video(
    videos: &PgVideoRepository,
    student: AccountId,
    coach: AccountId,
    submitted_at: SystemTime,
    returned_at: Option<SystemTime>,
) {
    let video_id = videos.next_video_id();
    videos
        .insert(Video::submit(
            video_id,
            student,
            coach,
            Sport::Tennis,
            submitted_at,
        ))
        .await
        .unwrap();
    if let Some(returned_at) = returned_at {
        let review_id = videos.next_review_id();
        videos
            .update(video_id, |video| {
                video.return_review(
                    review_id,
                    coach,
                    ReviewComment::new("よい").unwrap(),
                    true,
                    returned_at,
                )
            })
            .await
            .unwrap();
    }
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
    // 動画添削は直近 30 日に送付から 2・7・30 時間で 3 本を返し（中央値 7 時間）、31 日前に 1 本を返し、1 本はまだ返していない
    let videos = PgVideoRepository::new(pool.clone());
    let day = HOUR * 24;
    for (returned_at, lead) in [
        (now - HOUR, HOUR * 2),
        (now - day * 3, HOUR * 7),
        (now - day * 29, HOUR * 30),
        (now - day * 31, HOUR),
    ] {
        video(&videos, taro, tanaka, returned_at - lead, Some(returned_at)).await;
    }
    video(&videos, taro, tanaka, now - HOUR, None).await;
    video(&videos, taro, ito, now - HOUR * 5, Some(now - HOUR)).await;
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
    let expected = json!({
        "coach_id": tanaka_id,
        "display_name": "tanaka",
        "acceptance_status": "suspended",
        "review_count": 2,
        "average_rating": 4.5,
        "current_month": {
            "month": "2026-10",
            "live_lesson_count": 0,
            "async_lesson_count": 0,
            "review_count": 1,
            "average_rating": 4.0
        },
        "previous_month": {
            "month": "2026-09",
            "live_lesson_count": 0,
            "async_lesson_count": 0,
            "review_count": 1,
            "average_rating": 5.0
        },
        "published_lesson_menu_count": 2,
        "returned_video_review_count": 3,
        "median_video_review_lead_time_hours": 7.0
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
    assert_eq!(body["published_lesson_menu_count"], 0);
    assert_eq!(body["returned_video_review_count"], 1);
    assert_eq!(body["median_video_review_lead_time_hours"], 4.0);

    let student_path = format!("/coaches/{}/overview", account_id_uuid::format(taro));
    let (status, _) = send(&router, "GET", &student_path, &hq_token, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    for token in [&tanaka_token, &taro_token] {
        let (status, _) = send(&router, "GET", &path, token, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
}
