//! 本部が見る低い評価の一覧（admin-web の `GET /lesson-reviews/low`）を、保存先を Postgres にした組み立て
//! （`server::router_with_postgres`）で通す。直近 30 日の★2 以下の評価だけがコーチを問わず新しい順に並び、コーチ・生徒の
//! 表示名がアカウントの表から添わること、コーチ・生徒は 403 であることを確かめる。`sport` を付けると、評価した完了した
//! レッスンの元（同期は予約の枠、非同期は動画）のスポーツで絞られることも確かめる。本部が `PUT /lesson-reviews/{review_id}/handling`
//! で対応済みの印を付けると Postgres に残り（2 回目は最初の印のまま）、一覧の `handled=false` で未対応だけに絞れることも確かめる。
//! 契約の形の細かい検査は interface の `tests/low_rated_reviews.rs`・`tests/low_rated_review_handlings.rs`。
//!
//! 評価は完了したレッスンが要り、そこまでの HTTP の流れは `lessons.rs` で通しているので、ここでは枠・予約・通話セッション・
//! 動画・完了記録・評価を保存先に直に書く（`review.lesson_reviews` の `lesson_id` には外部キーがない）。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test low_rated_reviews -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    async_lesson::VideoRepository,
    identity::{
        RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer, account_id_uuid,
    },
    in_memory::{FixedClock, RecordingAnalyticsSink},
    lesson::LessonCompletionRepository,
    live_lesson::LiveLessonSessionRepository,
    review::{LessonReviewRepository, lesson_review_id_uuid},
    scheduling::{BookingRepository, SlotRepository},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    async_lesson::{ReviewComment, Video, VideoReviewId},
    catalog::Sport,
    identity::{AccountId, Role},
    lesson::{LessonCompletion, LessonCompletionId},
    live_lesson::{ConnectionQuality, LiveLessonSession},
    review::{LessonReview, LessonReviewComment, Rating},
    scheduling::{Booking, Slot, SlotCapacity},
};
use infra::{
    MockTokenService, PgAccountRepository, PgBookingRepository, PgLessonCompletionRepository,
    PgLessonReviewRepository, PgLiveLessonSessionRepository, PgSlotRepository, PgVideoRepository,
};
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
    get_with_query(router, token, "").await
}

/// `GET /lesson-reviews/low` にクエリ `query`（`?` から。なければ空）を付ける。
async fn get_with_query(router: &Router, token: &str, query: &str) -> (StatusCode, Value) {
    let request = Request::get(format!("/lesson-reviews/low{query}"))
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

/// コーチ `coach` のテニスの枠（`at` から 1 時間）を生徒 `student` が予約・確定し、通話して完了した同期レッスンを保存先に書く。
async fn live_lesson(
    pool: &PgPool,
    coach: AccountId,
    student: AccountId,
    at: SystemTime,
) -> LessonCompletion {
    let slots = PgSlotRepository::new(pool.clone());
    let slot = Slot::publish(
        slots.next_id(),
        coach,
        Sport::Tennis,
        at,
        at + Duration::from_secs(3600),
        SlotCapacity::new(1).unwrap(),
    )
    .unwrap();
    let slot_id = slot.id();
    slots
        .add_to_calendar(slot, |calendar, slot| calendar.publish(slot))
        .await
        .unwrap();
    let bookings = PgBookingRepository::new(pool.clone());
    let booking_id = bookings.next_id();
    let reserved_at = at - 2 * DAY;
    bookings
        .reserve(slot_id, student, |slot, existing| {
            Booking::reserve(booking_id, student, false, slot, existing, reserved_at)
        })
        .await
        .unwrap();
    let booking = bookings
        .update(booking_id, |booking, slot| {
            booking.confirm(student, slot, reserved_at)
        })
        .await
        .unwrap();
    let sessions = PgLiveLessonSessionRepository::new(pool.clone());
    let session = LiveLessonSession::start(
        sessions.next_id(),
        coach,
        &booking,
        std::slice::from_ref(&booking),
        None,
        at,
    )
    .unwrap();
    let session_id = session.id();
    sessions.insert(session).await.unwrap();
    let ended_at = at + Duration::from_secs(3600);
    let session = sessions
        .update(session_id, |session| {
            session.end(
                coach,
                ended_at,
                ConnectionQuality::new(0.0, 10, 50, 0).unwrap(),
            )
        })
        .await
        .unwrap();
    let completions = PgLessonCompletionRepository::new(pool.clone());
    let id = completions.next_id();
    completions
        .record(student, |history| {
            session.complete_for(id, coach, student, history, ended_at)
        })
        .await
        .unwrap()
}

/// 生徒 `student` がコーチ `coach` に送った水泳の動画に `at` に添削が返り、完了した非同期レッスンを保存先に書く。
async fn async_lesson(
    pool: &PgPool,
    coach: AccountId,
    student: AccountId,
    at: SystemTime,
) -> LessonCompletion {
    let videos = PgVideoRepository::new(pool.clone());
    let video_id = videos.next_video_id();
    videos
        .insert(
            Video::restore(
                video_id,
                student,
                coach,
                Sport::Swimming,
                at - DAY,
                None,
                None,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let video = videos
        .update(video_id, |video| {
            video.return_review(
                VideoReviewId::from_u128(video_id.as_u128()),
                coach,
                ReviewComment::new("腕の入りを直そう").unwrap(),
                true,
                at,
            )
        })
        .await
        .unwrap();
    let completions = PgLessonCompletionRepository::new(pool.clone());
    let id = completions.next_id();
    completions
        .record(student, |history| video.complete(id, student, history, at))
        .await
        .unwrap()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_narrows_low_reviews_to_a_sport_from_postgres(pool: PgPool) {
    let (_, headquarters) = register(&pool, "honbu@example.com", Role::Headquarters).await;
    let (tanaka, _) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (taro, _) = register(&pool, "taro@example.com", Role::Student).await;

    let live = live_lesson(&pool, tanaka, taro, now() - 10 * DAY).await;
    let video = async_lesson(&pool, tanaka, taro, now() - 5 * DAY).await;
    let reviews = PgLessonReviewRepository::new(pool.clone());
    let tennis = review(
        &reviews,
        taro,
        tanaka,
        live.id().as_u128(),
        1,
        now() - 9 * DAY,
    )
    .await;
    let swimming = review(
        &reviews,
        taro,
        tanaka,
        video.id().as_u128(),
        2,
        now() - 4 * DAY,
    )
    .await;
    // 完了したレッスンがない評価（スポーツが引けない）は、絞ったときにどのスポーツにも載らない
    let unknown = review(&reviews, taro, tanaka, 999, 1, now() - DAY).await;
    let router =
        server::router_with_postgres(pool, FixedClock(now()), RecordingAnalyticsSink::default());

    let ids = |body: &Value| -> Vec<String> {
        body["reviews"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["review_id"].as_str().unwrap().to_string())
            .collect()
    };
    let (status, body) = get(&router, &headquarters).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        ids(&body),
        [unknown.as_str(), swimming.as_str(), tennis.as_str()],
        "付けなければ今までどおり"
    );

    for (sport, expected) in [
        ("tennis", vec![tennis.as_str()]),
        ("swimming", vec![swimming.as_str()]),
        ("golf", vec![]),
    ] {
        let (status, body) =
            get_with_query(&router, &headquarters, &format!("?sport={sport}")).await;
        assert_eq!(status, StatusCode::OK, "{sport}: {body}");
        assert_eq!(ids(&body), expected, "{sport}");
    }

    let (status, body) = get_with_query(&router, &headquarters, "?sport=pingpong").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");
}

/// `PUT /lesson-reviews/{review_id}/handling`（本文なし）。
async fn put_handling(router: &Router, token: &str, review_id: &str) -> (StatusCode, Value) {
    let request = Request::put(format!("/lesson-reviews/{review_id}/handling"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_marks_low_reviews_handled_and_lists_only_the_unhandled_from_postgres(
    pool: PgPool,
) {
    let (honbu, headquarters) = register(&pool, "honbu@example.com", Role::Headquarters).await;
    let (_, other_headquarters) = register(&pool, "honbu2@example.com", Role::Headquarters).await;
    let (tanaka, tanaka_token) = register(&pool, "tanaka@example.com", Role::Coach).await;
    let (taro, taro_token) = register(&pool, "taro@example.com", Role::Student).await;

    let reviews = PgLessonReviewRepository::new(pool.clone());
    let older = review(&reviews, taro, tanaka, 1, 1, now() - 20 * DAY).await;
    let newer = review(&reviews, taro, tanaka, 2, 2, now() - DAY).await;
    let good = review(&reviews, taro, tanaka, 3, 3, now() - DAY).await;
    let router =
        server::router_with_postgres(pool, FixedClock(now()), RecordingAnalyticsSink::default());

    let (status, body) = put_handling(&router, &headquarters, &older).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["handled_by"], account_id_uuid::format(honbu));
    assert_eq!(body["handled_at"], "2026-10-31T00:00:00Z");
    // 2 回目（別の本部のアカウント）も同じ結果
    let (status, again) = put_handling(&router, &other_headquarters, &older).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again, body);

    let ids = |body: &Value| -> Vec<String> {
        body["reviews"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["review_id"].as_str().unwrap().to_string())
            .collect()
    };
    let (status, list) = get_with_query(&router, &headquarters, "?handled=false").await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(ids(&list), [newer.as_str()], "未対応だけ");
    assert_eq!(list["reviews"][0]["handling"], Value::Null);
    let (status, list) = get(&router, &headquarters).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(ids(&list), [newer.as_str(), older.as_str()]);
    assert_eq!(
        list["reviews"][1]["handling"], body,
        "一覧にも最初の印が載る"
    );

    let (status, body) = put_handling(&router, &headquarters, &good).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "lesson_review_not_low_rated");
    let (status, body) = put_handling(
        &router,
        &headquarters,
        "00000000-0000-0000-0000-000000000063",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    for token in [&tanaka_token, &taro_token] {
        let (status, body) = put_handling(&router, token, &newer).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
    let (_, list) = get_with_query(&router, &headquarters, "?handled=false").await;
    assert_eq!(ids(&list), [newer.as_str()], "拒んだ操作は何も変えない");
}
