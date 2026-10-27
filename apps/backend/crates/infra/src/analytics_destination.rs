//! 行動イベントの配信先を環境変数で選ぶ（server と seed が共有する）。

use application::{
    analytics::{AnalyticsEvent, AnalyticsSink},
    in_memory::RecordingAnalyticsSink,
};

use crate::{AnalyticsCommonAttributes, ObjectStoreAnalyticsSink};

/// 行動イベントの配信先。`ANALYTICS_SINK` で選ぶ。
///
/// | `ANALYTICS_SINK` | 配信先 |
/// |---|---|
/// | なし・`memory` | プロセスの中に記録するだけ（[`RecordingAnalyticsSink`]） |
/// | `object_store` | S3 API のオブジェクトストアに JSON Lines で書く（[`ObjectStoreAnalyticsSink`]）。`ANALYTICS_OBJECT_STORE_ENDPOINT`（既定 `http://127.0.0.1:9090`。compose の `objectstore`）の `ANALYTICS_OBJECT_STORE_BUCKET`（既定 `rally-analytics`） |
///
/// `object_store` では、各行に共通の任意属性（events.yaml の `common_optional`）を付ける。`ANALYTICS_RELEASE_ID` が
/// `release_id`、`ANALYTICS_APP_VERSION` が `app_version` で、ないか空なら付けない
/// （[`AnalyticsCommonAttributes::from_vars`]）。`memory` はイベントをそのまま記録するので属性の行を作らない。
#[derive(Debug, Clone)]
pub enum AnalyticsDestination {
    Memory(RecordingAnalyticsSink),
    ObjectStore(ObjectStoreAnalyticsSink),
}

impl AnalyticsDestination {
    pub const SINK_VAR: &'static str = "ANALYTICS_SINK";
    pub const ENDPOINT_VAR: &'static str = "ANALYTICS_OBJECT_STORE_ENDPOINT";
    pub const BUCKET_VAR: &'static str = "ANALYTICS_OBJECT_STORE_BUCKET";
    pub const DEFAULT_ENDPOINT: &'static str = "http://127.0.0.1:9090";
    pub const DEFAULT_BUCKET: &'static str = "rally-analytics";

    /// プロセスの環境変数から選ぶ。
    pub fn from_env() -> Result<Self, String> {
        Self::from_vars(|name| std::env::var(name).ok())
    }

    /// `var`（環境変数の名前から値）で選ぶ。知らない `ANALYTICS_SINK` の値は失敗にする（黙って既定に倒さない）。
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        match var(Self::SINK_VAR).as_deref() {
            None | Some("memory") => Ok(Self::Memory(RecordingAnalyticsSink::default())),
            Some("object_store") => {
                let endpoint =
                    var(Self::ENDPOINT_VAR).unwrap_or_else(|| Self::DEFAULT_ENDPOINT.to_string());
                let bucket =
                    var(Self::BUCKET_VAR).unwrap_or_else(|| Self::DEFAULT_BUCKET.to_string());
                let common = AnalyticsCommonAttributes::from_vars(&var);
                ObjectStoreAnalyticsSink::s3(&endpoint, &bucket)
                    .map(|sink| Self::ObjectStore(sink.with_common(common)))
                    .map_err(|e| format!("行動イベントの配信先 {endpoint} を組み立てられない: {e}"))
            }
            Some(other) => Err(format!(
                "{} は memory か object_store（{other:?} は知らない）",
                Self::SINK_VAR
            )),
        }
    }
}

impl AnalyticsSink for AnalyticsDestination {
    async fn emit(&self, event: AnalyticsEvent) {
        match self {
            Self::Memory(sink) => sink.emit(event).await,
            Self::ObjectStore(sink) => sink.emit(event).await,
        }
    }
}
