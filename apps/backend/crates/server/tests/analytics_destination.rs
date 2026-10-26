//! 行動イベントの配信先を環境変数で選ぶ（`AnalyticsDestination`）。

use std::collections::HashMap;

use server::AnalyticsDestination;

fn choose(vars: &[(&str, &str)]) -> Result<AnalyticsDestination, String> {
    let vars: HashMap<String, String> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    AnalyticsDestination::from_vars(|name| vars.get(name).cloned())
}

#[test]
fn memory_is_the_default() {
    assert!(matches!(choose(&[]), Ok(AnalyticsDestination::Memory(_))));
    assert!(matches!(
        choose(&[("ANALYTICS_SINK", "memory")]),
        Ok(AnalyticsDestination::Memory(_))
    ));
}

#[test]
fn object_store_uses_the_endpoint_and_the_bucket_of_the_variables() {
    assert!(matches!(
        choose(&[("ANALYTICS_SINK", "object_store")]),
        Ok(AnalyticsDestination::ObjectStore(_))
    ));
    assert!(matches!(
        choose(&[
            ("ANALYTICS_SINK", "object_store"),
            ("ANALYTICS_OBJECT_STORE_ENDPOINT", "http://objectstore:9090"),
            ("ANALYTICS_OBJECT_STORE_BUCKET", "other-bucket"),
        ]),
        Ok(AnalyticsDestination::ObjectStore(_))
    ));
}

#[test]
fn unknown_sink_is_an_error() {
    for value in ["", "firebase", "Memory", "s3"] {
        let error = choose(&[("ANALYTICS_SINK", value)]).unwrap_err();
        assert!(error.contains("ANALYTICS_SINK"), "{error}");
    }
}

/// `ANALYTICS_SINK=object_store` で組み立てたサーバーで生徒を登録すると、`account.registered` の 1 行が
/// S3 API の PUT で配信先のバケットに届く。配信先は PUT Object だけに答える偽物。
#[tokio::test]
async fn registering_a_student_puts_account_registered_into_the_object_store() {
    use std::sync::{Arc, Mutex};

    use application::clock::SystemClock;
    use axum::{
        Router,
        body::{Body, Bytes},
        http::{Request, StatusCode, Uri},
    };
    use serde_json::Value;
    use tower::ServiceExt;

    let received: Arc<Mutex<Vec<(String, String)>>> = Arc::default();
    let log = received.clone();
    let fake = Router::new().fallback(move |uri: Uri, body: Bytes| {
        let log = log.clone();
        async move {
            log.lock().unwrap().push((
                uri.path().replace("%3D", "="),
                String::from_utf8(body.to_vec()).unwrap(),
            ));
            (StatusCode::OK, [("etag", "\"mock\"")])
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, fake).await });

    let analytics = choose(&[
        ("ANALYTICS_SINK", "object_store"),
        ("ANALYTICS_OBJECT_STORE_ENDPOINT", &endpoint),
    ])
    .unwrap();
    let response = server::router_with_analytics(SystemClock, analytics)
        .oneshot(
            Request::post("/accounts")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"taro@example.com","display_name":"たろう","sport":"tennis"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let received = received.lock().unwrap().clone();
    assert_eq!(received.len(), 1, "{received:?}");
    let (path, body) = &received[0];
    assert!(
        path.starts_with("/rally-analytics/events/account.registered/dt="),
        "{path}"
    );
    assert!(
        body.ends_with('\n') && body.matches('\n').count() == 1,
        "{body:?}"
    );
    let line: Value = serde_json::from_str(body).unwrap();
    assert_eq!(line["name"], "account.registered");
    assert_eq!(line["attributes"]["sport"], "tennis");
}
