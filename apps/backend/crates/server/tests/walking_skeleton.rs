//! 一周シナリオ「体験予約 → 初回レッスン（通話・動画）→ 完了記録」の E2E（API レベル。段 7 の T19）。
//!
//! サーバーを infra の実装（保存先は Postgres、通話基盤・動画の保存先・アクセストークンはモック、境界間イベントは
//! インメモリのイベントバス）と、インメモリの行動イベントの配信先で組み立てて TCP で待ち受け、HTTP だけで通す。
//!
//! 1. コーチと生徒のアカウント登録 → コーチが枠を公開 → 生徒が検索して体験予約 → 確定
//! 2. 通話の開始 → 終了 → 完了記録（`is_first_lesson=true`）
//! 3. 同じ生徒が月額契約 → 動画を送付 → コーチが添削を返す → 完了記録（`is_first_lesson=false`）
//! 4. 同じ暦月に 3 回目の通話予約の確定が 409 `live_lesson_quota_exceeded` で拒否される
//!
//! 最後に、途中で発火した行動イベントの名前の並びが期待した列と一致することを確かめる。
//!
//! コーチのアカウント登録は契約に HTTP の操作がない（student-app の `registerAccount` は生徒だけ）ので、
//! 同じ Postgres に application のユースケース `RegisterAccount` で登録し、トークンはモックで発行する。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` がなければ skip ではなく失敗する。
//! `DATABASE_URL` を設定して `cargo test -p server --test walking_skeleton -- --include-ignored` で走らせる。

use std::{
    net::SocketAddr,
    time::{Duration, SystemTime},
};

use application::{
    identity::{RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer},
    in_memory::{ManualClock, RecordingAnalyticsSink},
};
use domain::identity::{Role, Sport};
use infra::{MockTokenService, PgAccountRepository};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

const MINUTE: Duration = Duration::from_secs(60);

/// 2026-10-05T00:00:00+09:00（2026-10-04T15:00:00Z）。シナリオの始まり
fn oct5_jst() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_791_126_000)
}

/// 2026-10-10T10:00:00Z（体験予約の枠の開始）
fn oct10_10() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_791_626_400)
}

/// TCP で待ち受けるサーバーへの HTTP/1.1 のクライアント（1 リクエストごとに接続を閉じる）。
struct Api {
    addr: SocketAddr,
}

impl Api {
    async fn request(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (u16, Value) {
        let body = body.map(|b| b.to_string()).unwrap_or_default();
        let mut head =
            format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n");
        if let Some(token) = token {
            head.push_str(&format!("Authorization: Bearer {token}\r\n"));
        }
        if !body.is_empty() {
            head.push_str("Content-Type: application/json\r\n");
        }
        head.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));

        let mut stream = TcpStream::connect(self.addr).await.unwrap();
        stream.write_all(head.as_bytes()).await.unwrap();
        stream.write_all(body.as_bytes()).await.unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();

        let (head, body) = response
            .split_once("\r\n\r\n")
            .unwrap_or_else(|| panic!("{method} {path}: HTTP の応答でない: {response:?}"));
        let status = head
            .split(' ')
            .nth(1)
            .and_then(|code| code.parse().ok())
            .unwrap_or_else(|| panic!("{method} {path}: ステータス行が読めない: {head:?}"));
        let body = serde_json::from_str(body)
            .unwrap_or_else(|e| panic!("{method} {path}: 本文が JSON でない（{e}）: {body:?}"));
        (status, body)
    }

    /// `expected` の status であることを確かめて本文を返す。
    async fn expect(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
        expected: u16,
    ) -> Value {
        let (status, body) = self.request(method, path, token, body).await;
        assert_eq!(status, expected, "{method} {path}: {body}");
        body
    }

    async fn post(&self, path: &str, token: &str, body: Value, expected: u16) -> Value {
        self.expect("POST", path, Some(token), Some(body), expected)
            .await
    }

    async fn get(&self, path: &str, token: &str) -> Value {
        self.expect("GET", path, Some(token), None, 200).await
    }
}

/// サーバーを `pool` の Postgres で組み立てて、空いているポートで待ち受ける。
async fn serve(pool: PgPool, clock: ManualClock, analytics: RecordingAnalyticsSink) -> Api {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = server::router_with_postgres(pool, clock, analytics);
    tokio::spawn(async move { axum::serve(listener, router).await });
    Api { addr }
}

/// コーチ（テニス）を登録してトークンを返す。契約にコーチの登録の操作がないので、ユースケースを直に呼ぶ。
async fn register_coach(
    pool: &PgPool,
    clock: &ManualClock,
    analytics: &RecordingAnalyticsSink,
) -> (String, String) {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        analytics.clone(),
        clock.clone(),
    )
    .execute(RegisterAccountCommand {
        email: "coach.hanako@example.com".to_string(),
        display_name: "はなこコーチ".to_string(),
        role: Role::Coach,
        sports: vec![Sport::Tennis],
    })
    .await
    .unwrap();
    let token = MockTokenService
        .issue(TokenClaims {
            account_id: account.id(),
            role: Role::Coach,
        })
        .into_string();
    let coach_id = application::identity::account_id_uuid::format(account.id());
    (coach_id, token)
}

fn slot(starts_at: &str, ends_at: &str) -> Value {
    json!({ "sport": "tennis", "starts_at": starts_at, "ends_at": ends_at })
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn trial_booking_to_first_live_lesson_then_async_lesson_and_the_quota(pool: PgPool) {
    let clock = ManualClock::new(oct5_jst());
    let analytics = RecordingAnalyticsSink::default();
    let (coach_id, coach) = register_coach(&pool, &clock, &analytics).await;
    let api = serve(pool, clock.clone(), analytics.clone()).await;

    // 1. 生徒の登録 → コーチが枠を公開 → 生徒が検索して体験予約 → 確定
    let account = api
        .expect(
            "POST",
            "/accounts",
            None,
            Some(
                json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "tennis" }),
            ),
            201,
        )
        .await;
    let student = account["access_token"].as_str().unwrap().to_string();
    let student_id = account["student"]["student_id"]
        .as_str()
        .unwrap()
        .to_string();

    let published = api
        .post(
            "/slots",
            &coach,
            slot("2026-10-10T10:00:00Z", "2026-10-10T11:00:00Z"),
            201,
        )
        .await;
    assert_eq!(published["status"], "open", "{published}");

    let page = api
        .get(
            &format!("/slots?sport=tennis&coach_id={coach_id}"),
            &student,
        )
        .await;
    let found = page["items"].as_array().unwrap();
    assert_eq!(found.len(), 1, "{page}");
    assert_eq!(found[0]["slot_id"], published["slot_id"]);

    let trial = api
        .post(
            "/bookings",
            &student,
            json!({ "slot_id": found[0]["slot_id"], "is_trial": true }),
            201,
        )
        .await;
    assert_eq!(trial["status"], "pending", "{trial}");
    assert_eq!(trial["is_trial"], true);
    assert_eq!(trial["coach_id"], coach_id.as_str());
    let trial_id = trial["booking_id"].as_str().unwrap();
    let confirmed = api
        .post(
            &format!("/bookings/{trial_id}/confirm"),
            &student,
            json!({}),
            200,
        )
        .await;
    assert_eq!(confirmed["status"], "confirmed", "{confirmed}");

    // 予約した枠は検索に出なくなる
    let page = api
        .get(
            &format!("/slots?sport=tennis&coach_id={coach_id}"),
            &student,
        )
        .await;
    assert_eq!(page["items"], json!([]), "{page}");

    // 2. 通話の開始 → 終了 → 完了記録（初回）
    clock.set(oct10_10());
    let session = api
        .post(
            "/live-lesson-sessions",
            &coach,
            json!({ "booking_id": trial_id }),
            201,
        )
        .await;
    assert_eq!(session["status"], "in_progress", "{session}");
    let session_id = session["session_id"].as_str().unwrap();
    clock.set(oct10_10() + 50 * MINUTE);
    let ended = api
        .post(
            &format!("/live-lesson-sessions/{session_id}/end"),
            &student,
            json!({}),
            200,
        )
        .await;
    assert_eq!(ended["status"], "ended", "{ended}");
    assert_eq!(ended["duration_sec"], 50 * 60);

    clock.set(oct10_10() + 55 * MINUTE);
    let first = api
        .post(
            "/lessons/completions",
            &student,
            json!({ "lesson_type": "live_lesson", "session_id": session_id }),
            201,
        )
        .await;
    assert_eq!(first["lesson_type"], "live_lesson", "{first}");
    assert_eq!(first["student_id"], student_id.as_str());
    assert_eq!(first["is_first_lesson"], true, "初回の完了記録");

    // 3. 月額契約 → 動画を送付 → コーチが添削を返す → 完了記録（2 回目）
    clock.set(oct10_10() + 2 * 60 * MINUTE);
    let subscribed = api
        .post(
            "/subscriptions",
            &student,
            json!({ "sport": "tennis", "payment_method_token": "tok_mock" }),
            201,
        )
        .await;
    assert_eq!(
        subscribed["subscription"]["status"], "active",
        "{subscribed}"
    );
    // 月額契約の前に確定した体験予約は枠を使わない
    assert_eq!(
        subscribed["entitlement"]["live_lesson_quota"],
        json!({ "month": "2026-10", "limit": 2, "used": 0, "remaining": 2 })
    );

    let video = api
        .post(
            "/videos",
            &student,
            json!({ "coach_id": coach_id, "sport": "tennis", "content_type": "video/mp4", "note": "フォアハンドを見てください" }),
            201,
        )
        .await;
    assert_eq!(video["status"], "awaiting_upload", "{video}");
    let video_id = video["video_id"].as_str().unwrap();
    let pending = api
        .get(&format!("/videos/{video_id}/review"), &student)
        .await;
    assert_eq!(pending["status"], "in_progress", "{pending}");

    clock.set(oct10_10() + 26 * 60 * MINUTE);
    let returned = api
        .post(
            &format!("/videos/{video_id}/review"),
            &coach,
            json!({ "comment": "テイクバックが早いので、ボールを引きつけてから振り始めましょう" }),
            200,
        )
        .await;
    assert_eq!(returned["status"], "returned", "{returned}");
    let review = api
        .get(&format!("/videos/{video_id}/review"), &student)
        .await;
    assert_eq!(review["status"], "returned", "{review}");
    assert_eq!(review["review_id"], returned["review_id"]);

    let second = api
        .post(
            "/lessons/completions",
            &student,
            json!({ "lesson_type": "async_lesson", "video_id": video_id }),
            201,
        )
        .await;
    assert_eq!(second["lesson_type"], "async_lesson", "{second}");
    assert_eq!(second["is_first_lesson"], false, "同じ生徒の 2 回目");

    // 4. 同じ暦月（2026-10）に 3 回目の通話予約の確定は拒否される
    let mut bookings = Vec::new();
    for day in [20, 21, 22] {
        let published = api
            .post(
                "/slots",
                &coach,
                slot(
                    &format!("2026-10-{day}T10:00:00Z"),
                    &format!("2026-10-{day}T11:00:00Z"),
                ),
                201,
            )
            .await;
        let booking = api
            .post(
                "/bookings",
                &student,
                json!({ "slot_id": published["slot_id"], "is_trial": false }),
                201,
            )
            .await;
        bookings.push(booking["booking_id"].as_str().unwrap().to_string());
    }
    for booking_id in &bookings[..2] {
        api.post(
            &format!("/bookings/{booking_id}/confirm"),
            &student,
            json!({}),
            200,
        )
        .await;
    }
    let rejected = api
        .post(
            &format!("/bookings/{}/confirm", bookings[2]),
            &student,
            json!({}),
            409,
        )
        .await;
    assert_eq!(rejected["code"], "live_lesson_quota_exceeded", "{rejected}");
    let entitlement = api.get("/entitlements/me", &student).await;
    assert_eq!(
        entitlement["live_lesson_quota"],
        json!({ "month": "2026-10", "limit": 2, "used": 2, "remaining": 0 })
    );

    // 行動イベントの名前の並び（events.yaml の name）。コーチの登録は account.registered を発火しない。
    // 拒否された確定は何も発火しない
    let names: Vec<&str> = analytics.events().iter().map(|e| e.name()).collect();
    assert_eq!(
        names,
        [
            // 1
            "account.registered",
            "slot.published",
            "booking.created",
            "booking.confirmed",
            // 2
            "call.started",
            "call.ended",
            "lesson.completed",
            // 3
            "subscription.started",
            "video.submitted",
            "video_review.returned",
            "lesson.completed",
            // 4
            "slot.published",
            "booking.created",
            "slot.published",
            "booking.created",
            "slot.published",
            "booking.created",
            "booking.confirmed",
            "booking.confirmed",
        ]
    );
}
