//! 本部が返した動画添削の記録を見る（admin-web の `listReturnedVideoReviews`）ことを、保存先を Postgres にした
//! 組み立て（`server::router_with_postgres`）で通す。ルートが組み立てに入っていること、Postgres の動画のうち返却の日付（日本時間）が
//! 期間に入るものをコーチを問わず返却の新しい順に読み、かかった時間とコーチ・生徒の表示名を添えること、`coach_id` で絞れること、
//! 期間の違反は 422、本部以外は 403 になることを確かめる。
//! 契約の形・クエリの検証の細かい検査は interface の `tests/returned_video_reviews.rs`、期間に返却した動画の読み取りは
//! infra の `tests/pg_video_repository.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test returned_video_reviews -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    async_lesson::VideoRepository,
    clock::SystemClock,
    identity::{
        RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer, account_id_uuid,
    },
    in_memory::{FixedClock, RecordingAnalyticsSink},
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
};
use infra::{MockTokenService, PgAccountRepository, PgVideoRepository};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

const HOUR: Duration = Duration::from_secs(3600);

/// 照会する時刻（2026-10-31T00:00:00Z。日本時間で 10-31 の 9 時）。
fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
}

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn get(path: &str, token: &str) -> Request<Body> {
    Request::get(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
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

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_reads_returned_video_reviews_from_postgres(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (ito, _) = register(&pool, "ito@example.com", Role::Coach).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;
    let (_, hq_token) = register(&pool, "hq@example.com", Role::Headquarters).await;

    // 動画の送付と返却は契約の操作を通すと時刻を選べないので、保存先に直に入れる
    let videos = PgVideoRepository::new(pool.clone());
    let submit = |coach, sport, hours_ago: u32| {
        Video::submit(
            videos.next_video_id(),
            taro,
            coach,
            sport,
            now() - HOUR * hours_ago,
        )
    };
    let (quick, slow_golf, pending) = (
        submit(tanaka, Sport::Tennis, 5),
        submit(ito, Sport::Golf, 50),
        submit(tanaka, Sport::Tennis, 100),
    );
    for video in [&quick, &slow_golf, &pending] {
        videos.insert(video.clone()).await.unwrap();
    }
    for (video, coach, returned_hours_ago) in [(&quick, tanaka, 1), (&slow_golf, ito, 2)] {
        videos
            .update(video.id(), |v| {
                v.return_review(
                    videos.next_review_id(),
                    coach,
                    ReviewComment::new("テイクバックが早い").unwrap(),
                    true,
                    now() - HOUR * returned_hours_ago,
                )
            })
            .await
            .unwrap();
    }

    let router =
        server::router_with_postgres(pool, FixedClock(now()), RecordingAnalyticsSink::default());

    let (status, body) = send(&router, get("/video-reviews/returned", &hq_token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let video_id = |video: &Video| application::async_lesson::video_id_uuid::format(video.id());
    assert_eq!(
        body,
        json!({
            "from": "2026-10-25",
            "to": "2026-10-31",
            "reviews": [
                {
                    "video_id": video_id(&quick),
                    "sport": "tennis",
                    "student_id": account_id_uuid::format(taro),
                    "student_display_name": "taro",
                    "coach_id": account_id_uuid::format(tanaka),
                    "coach_display_name": "tanaka",
                    "submitted_at": "2026-10-30T19:00:00Z",
                    "returned_at": "2026-10-30T23:00:00Z",
                    "lead_time_hours": 4,
                },
                {
                    "video_id": video_id(&slow_golf),
                    "sport": "golf",
                    "student_id": account_id_uuid::format(taro),
                    "student_display_name": "taro",
                    "coach_id": account_id_uuid::format(ito),
                    "coach_display_name": "ito",
                    "submitted_at": "2026-10-28T22:00:00Z",
                    "returned_at": "2026-10-30T22:00:00Z",
                    "lead_time_hours": 48,
                },
            ],
        }),
        "返していない動画は載らない"
    );

    let (status, body) = send(
        &router,
        get(
            &format!(
                "/video-reviews/returned?coach_id={}",
                account_id_uuid::format(ito)
            ),
            &hq_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["reviews"].as_array().unwrap().len(), 1);
    assert_eq!(body["reviews"][0]["video_id"], video_id(&slow_golf));

    // 返却はどれも日本時間の 10-31 なので、10-30 までの期間には入らない
    let (status, body) = send(
        &router,
        get(
            "/video-reviews/returned?from=2026-10-01&to=2026-10-30",
            &hq_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["reviews"], json!([]));

    let (status, body) = send(
        &router,
        get(
            "/video-reviews/returned?from=2026-10-01&to=2026-11-01",
            &hq_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");

    for token in [&tanaka_token, &taro_token] {
        let (status, body) = send(&router, get("/video-reviews/returned", token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
}
