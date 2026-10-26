//! 行動トラッキング（Firebase / GA 相当）の配信先のモック（Gen1 の外部連携はすべてモック）。

use std::{sync::Arc, time::Duration};

use application::analytics::{AnalyticsEvent, AnalyticsSink};
use object_store::{
    ClientOptions, ObjectStore, PutOptions, RetryConfig, aws::AmazonS3Builder, path::Path,
};
use serde_json::Value;
use uuid::Uuid;

use crate::analytics_event_payload::{attributes, line};

/// [`AnalyticsSink`] のモック。イベントを JSON Lines（[`line`] の 1 行と改行）でオブジェクトストア（S3 API）に書く。
/// ローカルのファイルシステムには何も残さない。
///
/// 1 回の発火で 1 オブジェクトを書き、キーは `events/<name>/dt=<occurred_at の UTC の日付>/<UUID v4>.jsonl`
/// （集計側がイベント名と日付で絞れるように）。書けなければ標準エラー出力に書いて捨てる（発火の失敗でユースケースを
/// 失敗させない）。
#[derive(Debug, Clone)]
pub struct ObjectStoreAnalyticsSink {
    store: Arc<dyn ObjectStore>,
}

impl ObjectStoreAnalyticsSink {
    /// キーの先頭。
    pub const KEY_PREFIX: &'static str = "events";

    pub fn new(store: Arc<dyn ObjectStore>) -> Self {
        Self { store }
    }

    /// S3 API のオブジェクトストア（ローカルの compose の `objectstore`）の `bucket` に書く。`endpoint` は
    /// `http://127.0.0.1:9090` の形で、バケットはパス形式（`<endpoint>/<bucket>/<key>`）で指す。
    ///
    /// モックの配信先なので認証情報は持たず、署名しない（`skip_signature`）。秘密情報が要る本物の配信先は
    /// Gen1 の範囲外。
    ///
    /// 発火はユースケースの中で待つので、配信先が止まっていても応答が長く止まらないよう、タイムアウト（接続 1 秒・
    /// 全体 2 秒）と再試行（1 回まで・合わせて 3 秒）を既定より短くする。
    pub fn s3(endpoint: &str, bucket: &str) -> Result<Self, object_store::Error> {
        let store = AmazonS3Builder::new()
            .with_endpoint(endpoint)
            .with_bucket_name(bucket)
            .with_region("us-east-1")
            .with_skip_signature(true)
            .with_client_options(
                ClientOptions::new()
                    .with_allow_http(true)
                    .with_connect_timeout(Duration::from_secs(1))
                    .with_timeout(Duration::from_secs(2)),
            )
            .with_retry(RetryConfig {
                max_retries: 1,
                retry_timeout: Duration::from_secs(3),
                ..RetryConfig::default()
            })
            .build()?;
        Ok(Self::new(Arc::new(store)))
    }

    /// `event` を書くキー。
    pub fn key(event: &AnalyticsEvent, id: Uuid) -> Result<Path, time::error::Format> {
        let attributes = attributes(event)?;
        // RFC 3339 の先頭 10 文字は日付（`YYYY-MM-DD`）。時刻は UTC で書く（`domain_event_payload::date_time`）
        let date = match &attributes["occurred_at"] {
            Value::String(at) => at[..10].to_string(),
            other => unreachable!("occurred_at は文字列: {other}"),
        };
        Ok(Path::from(format!(
            "{}/{}/dt={date}/{}.jsonl",
            Self::KEY_PREFIX,
            event.name(),
            id.hyphenated()
        )))
    }

    async fn put(&self, event: &AnalyticsEvent) -> Result<(), String> {
        let key = Self::key(event, Uuid::new_v4()).map_err(|e| e.to_string())?;
        let body = format!("{}\n", line(event).map_err(|e| e.to_string())?);
        self.store
            .put_opts(&key, body.into(), PutOptions::default())
            .await
            .map_err(|e| format!("{key}: {e}"))?;
        Ok(())
    }
}

impl AnalyticsSink for ObjectStoreAnalyticsSink {
    async fn emit(&self, event: AnalyticsEvent) {
        if let Err(error) = self.put(&event).await {
            eprintln!("analytics_event {} を書けない: {error}", event.name());
        }
    }
}
