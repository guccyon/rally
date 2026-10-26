//! 統合テストの共通部品: レスポンスを OpenAPI（specs/contracts/api/<アプリ>/openapi.yaml）の
//! スキーマで検査する。このテストが使う範囲のキーワードだけを扱い、扱えないものが出たら落とす。

use application::identity::account_id_uuid;
use axum::http::StatusCode;
use serde_json::Value;
use serde_yaml::Value as Yaml;

pub struct Contract {
    root: Yaml,
}

impl Contract {
    /// `app` は `specs/contracts/api/` の枝の名前（student-app / admin-web）。
    pub fn load(app: &str) -> Self {
        let path = format!(
            "{}/../../../../specs/contracts/api/{app}/openapi.yaml",
            env!("CARGO_MANIFEST_DIR")
        );
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        Self {
            root: serde_yaml::from_str(&text).unwrap(),
        }
    }

    /// `method path` の `status` の応答の形に `body` が合うか。契約にない status なら落とす。
    pub fn assert_response(&self, method: &str, path: &str, status: StatusCode, body: &Value) {
        let operation = &self.root["paths"][path][method];
        assert!(!operation.is_null(), "契約に {method} {path} がない");
        let response = operation["responses"]
            .get(status.as_str())
            .unwrap_or_else(|| panic!("契約の {method} {path} に {status} の応答がない"));
        let schema = &self.resolve(response)["content"]["application/json"]["schema"];
        self.assert_matches(schema, body, "$");
    }

    fn resolve<'a>(&'a self, node: &'a Yaml) -> &'a Yaml {
        match node.get("$ref").and_then(Yaml::as_str) {
            Some(reference) => {
                let path = reference.strip_prefix("#/").expect("内部参照だけを扱う");
                let target = path.split('/').fold(&self.root, |node, key| &node[key]);
                self.resolve(target)
            }
            None => node,
        }
    }

    fn assert_matches(&self, schema: &Yaml, value: &Value, at: &str) {
        let schema = self.resolve(schema);
        // `oneOf: [<スキーマ>, {type: "null"}]`（null を許す参照）だけを扱う
        if let Some(branches) = schema["oneOf"].as_sequence() {
            let is_null = |branch: &Yaml| self.resolve(branch)["type"].as_str() == Some("null");
            let others: Vec<&Yaml> = branches.iter().filter(|b| !is_null(b)).collect();
            assert!(
                others.len() == 1 && others.len() < branches.len(),
                "{at}: 検査できない oneOf"
            );
            if !value.is_null() {
                self.assert_matches(others[0], value, at);
            }
            return;
        }
        let ty = match &schema["type"] {
            // `type: [string, "null"]`
            Yaml::Sequence(types) => {
                if value.is_null() && types.iter().any(|t| t.as_str() == Some("null")) {
                    return;
                }
                types.iter().filter_map(Yaml::as_str).find(|t| *t != "null")
            }
            other => other.as_str(),
        };
        match ty {
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
                        self.assert_matches(child, v, &format!("{at}.{key}"));
                    }
                }
            }
            Some("array") => {
                let items = value
                    .as_array()
                    .unwrap_or_else(|| panic!("{at}: array でない: {value}"));
                for (i, item) in items.iter().enumerate() {
                    self.assert_matches(&schema["items"], item, &format!("{at}[{i}]"));
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
                    Some("uuid") => {
                        assert!(account_id_uuid::parse(s).is_ok(), "{at}: uuid でない {s}")
                    }
                    Some("date-time") => assert!(
                        time::OffsetDateTime::parse(
                            s,
                            &time::format_description::well_known::Rfc3339
                        )
                        .is_ok(),
                        "{at}: date-time（RFC 3339）でない {s}"
                    ),
                    // スキームと `://` のあとに何かがある最小の形（絶対 URI）
                    Some("uri") => assert!(
                        s.split_once("://").is_some_and(|(scheme, rest)| {
                            !rest.is_empty()
                                && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                                && scheme
                                    .chars()
                                    .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
                        }),
                        "{at}: uri でない {s}"
                    ),
                    Some(other) => panic!("{at}: 検査できない format {other}"),
                    None => {}
                }
            }
            Some("boolean") => assert!(value.is_boolean(), "{at}: boolean でない: {value}"),
            Some("integer") => {
                let n = value
                    .as_i64()
                    .unwrap_or_else(|| panic!("{at}: integer でない: {value}"));
                if let Some(c) = schema["const"].as_i64() {
                    assert_eq!(n, c, "{at}: const と違う");
                }
                if let Some(min) = schema["minimum"].as_i64() {
                    assert!(n >= min, "{at}: {n} < minimum {min}");
                }
                if let Some(max) = schema["maximum"].as_i64() {
                    assert!(n <= max, "{at}: {n} > maximum {max}");
                }
            }
            other => panic!("{at}: 検査できない type {other:?}"),
        }
    }
}
