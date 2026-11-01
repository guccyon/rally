//! 本部がコーチの一覧（admin-web の `listCoaches`）で直近 30 日に終わったレッスンの数を見ることを、保存先を Postgres にした
//! 組み立て（`server::router_with_postgres`）で通す。生徒が student-app の操作で動画添削を受けて完了を記録したことが、
//! 完了記録の保存先から読まれてコーチごとの数に出ること、照会した時刻から 30 日を過ぎると数えないこと、生徒には同じパスで
//! student-app の形（数を載せない）で返し、コーチは 403 であることを確かめる。期間の境目と契約の形の細かい検査は
//! interface の `tests/coaches.rs` と domain の `RecentLessonPeriod`。
//! あわせて、本部が表示名の検索語 `q` で Postgres に保存したコーチを探せること（ほかの絞り込みとの組み合わせと 422 を含む。
//! 一致の規則の細かい検査は domain の `DisplayNameQuery`）を確かめる。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test coaches -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    identity::{
        RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer, account_id_uuid,
    },
    in_memory::{ManualClock, RecordingAnalyticsSink},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{catalog::Sport, identity::Role};
use infra::{MockTokenService, PgAccountRepository};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

const DAY: u64 = 24 * 3600;

/// 2026-10-05T00:00:00Z から `secs` 秒後（Postgres の時刻はマイクロ秒までなので、秒単位にする）。
fn at(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_791_158_400 + secs)
}

async fn send(
    router: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
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

/// `token` で `path` に POST し、`expected` の status であることを確かめて本文を返す。
async fn post_ok(
    router: &Router,
    path: &str,
    token: &str,
    body: Value,
    expected: StatusCode,
) -> Value {
    let (status, body) = send(router, "POST", path, Some(token), Some(body)).await;
    assert_eq!(status, expected, "{path}: {body}");
    body
}

/// 役割 `role` のアカウントをユースケースを直に呼んで登録し（契約にコーチ・本部の登録の操作がない）、ID とトークンを返す。
async fn register(
    pool: &PgPool,
    email: &str,
    display_name: &str,
    role: Role,
    sports: Vec<Sport>,
) -> (String, String) {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        ManualClock::new(at(0)),
    )
    .execute(RegisterAccountCommand {
        email: email.to_string(),
        display_name: display_name.to_string(),
        role,
        sports,
    })
    .await
    .unwrap();
    let token = MockTokenService
        .issue(TokenClaims {
            account_id: account.id(),
            role,
        })
        .into_string();
    (account_id_uuid::format(account.id()), token)
}

/// 生徒 `student` がコーチ `coach_id` に動画を送り、`coach` が添削を返し、生徒が完了を記録する（今の時刻のまま）。
async fn complete_async_lesson(router: &Router, student: &str, coach_id: &str, coach: &str) {
    let video = post_ok(
        router,
        "/videos",
        student,
        json!({ "coach_id": coach_id, "sport": "tennis", "content_type": "video/mp4" }),
        StatusCode::CREATED,
    )
    .await;
    let video_id = video["video_id"].as_str().unwrap();
    post_ok(
        router,
        &format!("/videos/{video_id}/review"),
        coach,
        json!({ "comment": "フォロースルーを大きく" }),
        StatusCode::OK,
    )
    .await;
    post_ok(
        router,
        "/lessons/completions",
        student,
        json!({ "lesson_type": "async_lesson", "video_id": video_id }),
        StatusCode::CREATED,
    )
    .await;
}

/// 本部の一覧の `(表示名, recent_lesson_count)` を並びのまま。
fn counts(body: &Value) -> Vec<(String, Value)> {
    body["coaches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["display_name"].as_str().unwrap().to_string(),
                c["recent_lesson_count"].clone(),
            )
        })
        .collect()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_sees_lessons_completed_in_the_last_30_days_of_each_coach(pool: PgPool) {
    let (tanaka_id, tanaka) = register(
        &pool,
        "tanaka@example.com",
        "たなか",
        Role::Coach,
        vec![Sport::Tennis],
    )
    .await;
    let (ito_id, ito) = register(
        &pool,
        "ito@example.com",
        "いとう",
        Role::Coach,
        vec![Sport::Tennis],
    )
    .await;
    register(
        &pool,
        "ueno@example.com",
        "うえの",
        Role::Coach,
        vec![Sport::Tennis],
    )
    .await;
    let (_, headquarters) =
        register(&pool, "hq@example.com", "本部", Role::Headquarters, vec![]).await;
    let clock = ManualClock::new(at(60));
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    let (status, account) = send(
        &router,
        "POST",
        "/accounts",
        None,
        Some(json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "tennis" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{account}");
    let student = account["access_token"].as_str().unwrap().to_string();

    // 日 0（の 60 秒後）にいとうと 1 回、日 10 にたなかと 2 回・いとうと 1 回、動画添削を終える
    complete_async_lesson(&router, &student, &ito_id, &ito).await;
    clock.set(at(10 * DAY));
    complete_async_lesson(&router, &student, &tanaka_id, &tanaka).await;
    complete_async_lesson(&router, &student, &tanaka_id, &tanaka).await;
    complete_async_lesson(&router, &student, &ito_id, &ito).await;

    // 照会した時刻が日 0 のレッスンのちょうど 30 日後なら、日 0 の分も含む
    clock.set(at(60 + 30 * DAY));
    let (status, body) = send(&router, "GET", "/coaches", Some(&headquarters), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        counts(&body),
        [
            ("いとう".to_string(), json!(2)),
            ("うえの".to_string(), json!(0)),
            ("たなか".to_string(), json!(2)),
        ]
    );
    // 照会だけで何も変えない（2 回目も同じ）
    let (_, again) = send(&router, "GET", "/coaches", Some(&headquarters), None).await;
    assert_eq!(again, body);

    // 1 秒過ぎると日 0 の分は期間の外
    clock.set(at(61 + 30 * DAY));
    let (_, body) = send(&router, "GET", "/coaches", Some(&headquarters), None).await;
    assert_eq!(
        counts(&body),
        [
            ("いとう".to_string(), json!(1)),
            ("うえの".to_string(), json!(0)),
            ("たなか".to_string(), json!(2)),
        ]
    );
    // 絞っても数は同じ
    let (status, body) = send(
        &router,
        "GET",
        "/coaches?sport=tennis&acceptance_status=accepting",
        Some(&headquarters),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(counts(&body)[2], ("たなか".to_string(), json!(2)));

    // 生徒には student-app の形で返し、数は載せない。コーチは 403
    let (status, body) = send(&router, "GET", "/coaches", Some(&student), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for coach in body["coaches"].as_array().unwrap() {
        assert!(coach.get("recent_lesson_count").is_none(), "{coach}");
    }
    let (status, _) = send(&router, "GET", "/coaches", Some(&tanaka), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// コーチの一覧の表示名を、並びのまま取り出す。
fn names(body: &Value) -> Vec<String> {
    body["coaches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["display_name"].as_str().unwrap().to_string())
        .collect()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_searches_coaches_by_display_name(pool: PgPool) {
    let (yamada_id, _) = register(
        &pool,
        "yamada@example.com",
        "Taro Yamada",
        Role::Coach,
        vec![Sport::Tennis],
    )
    .await;
    for (email, name, sport) in [
        ("hanako@example.com", "山田 花子", Sport::Soccer),
        ("yamada2@example.com", "山田", Sport::Tennis),
        ("hira@example.com", "やまだ", Sport::Tennis),
        ("percent@example.com", "100%コーチ", Sport::Golf),
    ] {
        register(&pool, email, name, Role::Coach, vec![sport]).await;
    }
    register(
        &pool,
        "student@example.com",
        "山田 生徒",
        Role::Student,
        vec![Sport::Tennis],
    )
    .await;
    let (_, headquarters) =
        register(&pool, "hq@example.com", "本部", Role::Headquarters, vec![]).await;
    let router = server::router_with_postgres(
        pool,
        ManualClock::new(at(60)),
        RecordingAnalyticsSink::default(),
    );
    let (status, body) = send(
        &router,
        "POST",
        &format!("/coaches/{yamada_id}/suspend"),
        Some(&headquarters),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // 山田 = %E5%B1%B1%E7%94%B0、やまだ = %E3%82%84%E3%81%BE%E3%81%A0、全角の空白 = %E3%80%80
    for (query, expected) in [
        // 並べ方は表示名のコードポイントの順のまま。生徒は載らない
        ("?q=%E5%B1%B1%E7%94%B0", vec!["山田", "山田 花子"]),
        ("?q=%E3%80%80%E5%B1%B1%E7%94%B0+", vec!["山田", "山田 花子"]),
        // ASCII の英字の大文字・小文字は区別しない。ひらがなと漢字は同じとみなさない
        ("?q=YAMADA", vec!["Taro Yamada"]),
        ("?q=%E3%82%84%E3%81%BE%E3%81%A0", vec!["やまだ"]),
        // % はワイルドカードでなく文字のまま
        ("?q=%25", vec!["100%コーチ"]),
        ("?q=_", vec![]),
        // ほかの絞り込みと組み合わせる
        ("?q=%E5%B1%B1%E7%94%B0&sport=tennis", vec!["山田"]),
        ("?q=yamada&acceptance_status=accepting", vec![]),
        ("?q=yamada&acceptance_status=suspended", vec!["Taro Yamada"]),
        // 空白だけは絞らない
        (
            "?q=+",
            vec!["100%コーチ", "Taro Yamada", "やまだ", "山田", "山田 花子"],
        ),
    ] {
        let (status, body) = send(
            &router,
            "GET",
            &format!("/coaches{query}"),
            Some(&headquarters),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(names(&body), expected, "{query}");
    }

    let (status, body) = send(
        &router,
        "GET",
        &format!("/coaches?q={}", "a".repeat(51)),
        Some(&headquarters),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");
}
