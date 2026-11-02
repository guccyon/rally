//! コーチが自分に届いた動画を返却待ちだけに絞る（admin-web の `listMyCoachVideos` の `pending_only`）ことを、保存先を
//! Postgres にした組み立て（`server::router_with_postgres`）で通す。`pending_only=true` なら Postgres の動画のうち
//! 添削を返していないものだけを送られた時刻の古い順に返し、省いたとき・`false` は返却済みも含めて今までどおりに返すこと、
//! 別のコーチに送られた動画は出ないこと、値が `true` / `false` でなければ 422 になることを確かめる。
//! 契約の形の細かい検査は interface の `tests/videos.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test coach_videos -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    async_lesson::{VideoRepository, video_id_uuid},
    clock::SystemClock,
    identity::{RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer},
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
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

const HOUR: Duration = Duration::from_secs(3600);

/// 照会する時刻（2026-10-31T00:00:00Z）。
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
async fn coach_lists_only_videos_awaiting_review_from_postgres(pool: PgPool) {
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (ito, _) = register(&pool, "ito@example.com", Role::Coach).await;
    let (taro, _) = register(&pool, "taro@example.com", Role::Student).await;

    // 動画の送付は契約の操作を通すと時刻を選べないので、保存先に直に入れる
    let videos = PgVideoRepository::new(pool.clone());
    let submit = |coach, hours_ago: u32| {
        Video::submit(
            videos.next_video_id(),
            taro,
            coach,
            Sport::Tennis,
            now() - HOUR * hours_ago,
        )
    };
    let (recent, old, returned, others) = (
        submit(tanaka, 5),
        submit(tanaka, 50),
        submit(tanaka, 100),
        submit(ito, 10),
    );
    for video in [&recent, &old, &returned, &others] {
        videos.insert(video.clone()).await.unwrap();
    }
    videos
        .update(returned.id(), |v| {
            v.return_review(
                videos.next_review_id(),
                tanaka,
                ReviewComment::new("テイクバックが早い").unwrap(),
                true,
                now() - HOUR,
            )
        })
        .await
        .unwrap();

    let router =
        server::router_with_postgres(pool, FixedClock(now()), RecordingAnalyticsSink::default());
    let ids = |body: &Value| -> Vec<String> {
        body["videos"]
            .as_array()
            .unwrap()
            .iter()
            .map(|video| video["video_id"].as_str().unwrap().to_string())
            .collect()
    };
    let id = |video: &Video| video_id_uuid::format(video.id());

    // 返却待ちだけを送られた時刻の古い順に。返却済みと別のコーチの動画は出ない
    let (status, body) = send(
        &router,
        get("/coaches/me/videos?pending_only=true", &tanaka_token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(ids(&body), [id(&old), id(&recent)]);
    assert!(
        body["videos"]
            .as_array()
            .unwrap()
            .iter()
            .all(|video| video["review_status"] == "in_progress")
    );

    // 省いたとき・false は返却済みも後ろに含めて今までどおり
    let (status, all) = send(&router, get("/coaches/me/videos", &tanaka_token)).await;
    assert_eq!(status, StatusCode::OK, "{all}");
    assert_eq!(ids(&all), [id(&old), id(&recent), id(&returned)]);
    let (status, body) = send(
        &router,
        get("/coaches/me/videos?pending_only=false", &tanaka_token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, all);

    let (status, body) = send(
        &router,
        get("/coaches/me/videos?pending_only=yes", &tanaka_token),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");
}
