//! 行動イベントの共通の任意属性（specs/analytics/events.yaml の `common_optional`）。

/// すべての行動イベントに付ける共通の任意属性の値。値が分かるものだけを持ち、分からなければ `None`
/// （その属性を付けない。欠けていても発火は止めない）。JSON への書き方は [`crate::analytics_event_payload::line`]。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnalyticsCommonAttributes {
    /// `release_id`（版（Release）の ID。例: `guccyon/rally@0.1.0`）
    pub release_id: Option<String>,
    /// `app_version`（アプリの版。例: `0.1.0`）
    pub app_version: Option<String>,
}

impl AnalyticsCommonAttributes {
    pub const RELEASE_ID_VAR: &'static str = "ANALYTICS_RELEASE_ID";
    pub const APP_VERSION_VAR: &'static str = "ANALYTICS_APP_VERSION";

    /// `var`（環境変数の名前から値）で読む。ないか空（前後の空白だけを含む）なら分からないとして `None`。
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Self {
        let known = |name: &str| {
            var(name)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };
        Self {
            release_id: known(Self::RELEASE_ID_VAR),
            app_version: known(Self::APP_VERSION_VAR),
        }
    }
}
