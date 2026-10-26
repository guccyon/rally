//! `POST /accounts` の統合テスト。レスポンスの形を student-app の OpenAPI
//! （specs/contracts/api/student-app/openapi.yaml）のスキーマで検査する。

use std::time::{Duration, SystemTime};

use application::{
    analytics::AnalyticsEvent,
    identity::{
        AccessToken, InvalidToken, RegisterAccount, TokenClaims, TokenIssuer, TokenVerifier,
    },
    in_memory::{FixedClock, InMemoryAccountRepository, RecordingAnalyticsSink},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{Role, Sport};
use serde_json::{Value, json};
use serde_yaml::Value as Yaml;
use tower::ServiceExt;

const OPENAPI: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../specs/contracts/api/student-app/openapi.yaml"
);

/// 2026-10-26T00:00:00Z
const NOW: Duration = Duration::from_secs(1_792_972_800);

/// 発行した内容が読める形のトークン（interface は infra のモックに依存できないので、テスト用に置く）。
struct FakeTokens;

impl TokenIssuer for FakeTokens {
    fn issue(&self, claims: TokenClaims) -> AccessToken {
        AccessToken::new(format!(
            "fake:{}:{}",
            claims.role.as_str(),
            claims.account_id
        ))
    }
}

impl TokenVerifier for FakeTokens {
    fn verify(&self, _token: &str) -> Result<TokenClaims, InvalidToken> {
        Err(InvalidToken)
    }
}

struct App {
    router: Router,
    analytics: RecordingAnalyticsSink,
}

fn app() -> App {
    let analytics = RecordingAnalyticsSink::default();
    let router = interface::router_with(
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            analytics.clone(),
            FixedClock(SystemTime::UNIX_EPOCH + NOW),
        ),
        FakeTokens,
    );
    App { router, analytics }
}

async fn post_accounts(router: &Router, body: String) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(
            Request::post("/accounts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/json",
        "{status}"
    );
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn request(email: &str, display_name: &str, sport: &str) -> String {
    json!({ "email": email, "display_name": display_name, "sport": sport }).to_string()
}

// --- OpenAPI のスキーマによる検査（このテストが使う範囲のキーワードだけ） ---

fn openapi() -> Yaml {
    serde_yaml::from_str(&std::fs::read_to_string(OPENAPI).unwrap()).unwrap()
}

fn resolve<'a>(root: &'a Yaml, node: &'a Yaml) -> &'a Yaml {
    match node.get("$ref").and_then(Yaml::as_str) {
        Some(reference) => {
            let path = reference.strip_prefix("#/").expect("内部参照だけを扱う");
            let target = path.split('/').fold(root, |node, key| &node[key]);
            resolve(root, target)
        }
        None => node,
    }
}

/// `POST /accounts` の `status` の応答の JSON スキーマ。契約にない status なら落とす。
fn response_schema(root: &Yaml, status: StatusCode) -> &Yaml {
    let responses = &root["paths"]["/accounts"]["post"]["responses"];
    let response = responses
        .get(status.as_str())
        .unwrap_or_else(|| panic!("契約に {status} の応答がない"));
    resolve(
        root,
        &resolve(root, response)["content"]["application/json"]["schema"],
    )
}

fn assert_matches(root: &Yaml, schema: &Yaml, value: &Value, at: &str) {
    let schema = resolve(root, schema);
    match schema["type"].as_str() {
        Some("object") => {
            let object = value
                .as_object()
                .unwrap_or_else(|| panic!("{at}: object でない: {value}"));
            let properties = schema["properties"].as_mapping().unwrap();
            let declared: Vec<&str> = properties.keys().map(|k| k.as_str().unwrap()).collect();
            for key in object.keys() {
                assert!(
                    declared.contains(&key.as_str()),
                    "{at}: 契約にない項目 {key}"
                );
            }
            for key in schema["required"].as_sequence().into_iter().flatten() {
                let key = key.as_str().unwrap();
                assert!(object.contains_key(key), "{at}: 必須の項目 {key} がない");
            }
            for (key, child) in properties {
                let key = key.as_str().unwrap();
                if let Some(v) = object.get(key) {
                    assert_matches(root, child, v, &format!("{at}.{key}"));
                }
            }
        }
        Some("string") => {
            let s = value
                .as_str()
                .unwrap_or_else(|| panic!("{at}: string でない: {value}"));
            if let Some(values) = schema["enum"].as_sequence() {
                assert!(
                    values.iter().any(|v| v.as_str() == Some(s)),
                    "{at}: enum にない値 {s}"
                );
            }
            match schema["format"].as_str() {
                Some("uuid") => assert!(
                    s.parse::<domain::identity::AccountId>().is_ok(),
                    "{at}: uuid でない {s}"
                ),
                Some("date-time") => assert!(
                    time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339)
                        .is_ok(),
                    "{at}: date-time（RFC 3339）でない {s}"
                ),
                Some(other) => panic!("{at}: 検査できない format {other}"),
                None => {}
            }
        }
        other => panic!("{at}: 検査できない type {other:?}"),
    }
}

fn assert_contract(status: StatusCode, body: &Value) {
    let root = openapi();
    assert_matches(&root, response_schema(&root, status), body, "$");
}

// --- テスト ---

#[tokio::test]
async fn registers_a_student_with_the_contract_shape() {
    let app = app();

    let (status, body) =
        post_accounts(&app.router, request("taro@example.com", "たろう", "tennis")).await;

    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_contract(status, &body);
    let student = &body["student"];
    assert_eq!(student["display_name"], "たろう");
    assert_eq!(student["sport"], "tennis");
    assert_eq!(student["created_at"], "2026-10-26T00:00:00Z");
    assert_eq!(
        body["access_token"],
        format!(
            "fake:{}:{}",
            Role::Student.as_str(),
            student["student_id"].as_str().unwrap()
        ),
        "トークンは TokenIssuer が生徒の ID と役割で発行する"
    );

    let events = app.analytics.events();
    assert_eq!(events.len(), 1, "account.registered は 1 回だけ発火する");
    let AnalyticsEvent::AccountRegistered {
        student_id,
        sport,
        occurred_at,
    } = &events[0];
    assert_eq!(events[0].name(), "account.registered");
    assert_eq!(student_id.to_string(), student["student_id"]);
    assert_eq!(*sport, Sport::Tennis);
    assert_eq!(*occurred_at, SystemTime::UNIX_EPOCH + NOW);
}

#[tokio::test]
async fn accepts_every_student_app_sport() {
    let app = app();
    for (i, sport) in ["tennis", "baseball", "soccer", "basketball"]
        .iter()
        .enumerate()
    {
        let (status, body) = post_accounts(
            &app.router,
            request(&format!("s{i}@example.com"), "生徒", sport),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{sport}: {body}");
        assert_eq!(body["student"]["sport"], *sport);
    }
}

#[tokio::test]
async fn invalid_input_is_422_validation_failed_without_emitting() {
    let app = app();
    let cases = [
        (
            "未知のスポーツ",
            request("taro@example.com", "たろう", "cricket"),
        ),
        (
            "ゴルフ（golf-app の範囲）",
            request("taro@example.com", "たろう", "golf"),
        ),
        ("空の表示名", request("taro@example.com", "", "tennis")),
        (
            "空白だけの表示名",
            request("taro@example.com", "  ", "tennis"),
        ),
        (
            "51 文字の表示名",
            request("taro@example.com", &"あ".repeat(51), "tennis"),
        ),
        ("メールアドレスの形", request("taro", "たろう", "tennis")),
        (
            "必須項目の欠落",
            json!({ "email": "taro@example.com", "sport": "tennis" }).to_string(),
        ),
        (
            "型の違い",
            json!({ "email": "taro@example.com", "display_name": "たろう", "sport": 1 })
                .to_string(),
        ),
        ("JSON でない", "{".to_string()),
    ];

    for (case, body) in cases {
        let (status, body) = post_accounts(&app.router, body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{case}: {body}");
        assert_contract(status, &body);
        assert_eq!(body["code"], "validation_failed", "{case}");
    }
    assert!(app.analytics.events().is_empty());
}

#[tokio::test]
async fn duplicate_email_is_409_email_already_registered() {
    let app = app();
    let (status, _) =
        post_accounts(&app.router, request("taro@example.com", "たろう", "tennis")).await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, body) =
        post_accounts(&app.router, request("Taro@Example.com", "たろう", "soccer")).await;

    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_contract(status, &body);
    assert_eq!(body["code"], "email_already_registered");
    assert_eq!(app.analytics.events().len(), 1);
}
