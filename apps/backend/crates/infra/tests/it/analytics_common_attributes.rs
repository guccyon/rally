//! 行動イベントの共通の任意属性を環境変数から読む（`AnalyticsCommonAttributes::from_vars`）。

use std::collections::HashMap;

use infra::AnalyticsCommonAttributes;

fn read(vars: &[(&str, &str)]) -> AnalyticsCommonAttributes {
    let vars: HashMap<String, String> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    AnalyticsCommonAttributes::from_vars(|name| vars.get(name).cloned())
}

#[test]
fn values_are_unknown_without_the_variables() {
    assert_eq!(read(&[]), AnalyticsCommonAttributes::default());
}

#[test]
fn values_come_from_the_variables() {
    assert_eq!(
        read(&[
            ("ANALYTICS_RELEASE_ID", "guccyon/rally@0.1.0"),
            ("ANALYTICS_APP_VERSION", "0.1.0"),
        ]),
        AnalyticsCommonAttributes {
            release_id: Some("guccyon/rally@0.1.0".to_string()),
            app_version: Some("0.1.0".to_string()),
        }
    );
}

/// 片方だけ分かるときは、分かる方だけを持つ。空や空白だけの値は分からないとみなす。
#[test]
fn empty_values_are_unknown() {
    assert_eq!(
        read(&[
            ("ANALYTICS_RELEASE_ID", " "),
            ("ANALYTICS_APP_VERSION", " 0.1.0\n"),
        ]),
        AnalyticsCommonAttributes {
            release_id: None,
            app_version: Some("0.1.0".to_string()),
        }
    );
    assert_eq!(
        read(&[("ANALYTICS_RELEASE_ID", ""), ("ANALYTICS_APP_VERSION", "")]),
        AnalyticsCommonAttributes::default()
    );
}
