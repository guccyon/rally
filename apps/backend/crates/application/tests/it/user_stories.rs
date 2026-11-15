//! ユーザーストーリー（specs/stories/）の検査。型は factory-engine の definitions/user_story.yaml。
//!
//! 必須の欄があること、id とファイル名が揃うこと、利用者の種類（actors.yaml）と出てくる機能（touches）の id が
//! 正本（context-map.yaml・glossary.yaml・各アプリの openapi.yaml・analytics/events.yaml）にあることを確かめる。
//! 流れが実装で通るかはここでは見ない（QA 役がシナリオに落として流す）。

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_yaml::Value as Yaml;

const REQUIRED: [&str; 10] = [
    "type", "id", "title", "actor", "goal", "reader", "entry", "exit", "steps", "must_not",
];
const STATUSES: [&str; 3] = ["active", "draft", "retired"];

fn specs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../specs")
}

fn load(path: &PathBuf) -> Yaml {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn spec(relative: &str) -> Yaml {
    load(&specs_dir().join(relative))
}

fn seq<'a>(node: &'a Yaml, what: &str) -> &'a Vec<Yaml> {
    node.as_sequence()
        .unwrap_or_else(|| panic!("{what} が配列でない: {node:?}"))
}

fn str_of<'a>(node: &'a Yaml, what: &str) -> &'a str {
    node.as_str().unwrap_or_else(|| {
        panic!("{what} が文字列でない（YAML の true などは引用符で囲む）: {node:?}")
    })
}

/// 配列の各項目の `key`（文字列）。
fn ids(list: &Yaml, key: &str, what: &str) -> BTreeSet<String> {
    seq(list, what)
        .iter()
        .map(|item| str_of(&item[key], what).to_string())
        .collect()
}

fn actors() -> BTreeSet<String> {
    ids(
        &spec("stories/actors.yaml")["actors"],
        "id",
        "actors.yaml の actors",
    )
}

/// `<app>.<operationId>` の集合。
fn operations() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let dir = specs_dir().join("contracts/api");
    for app in std::fs::read_dir(&dir).unwrap() {
        let app = app.unwrap().path();
        let file = app.join("openapi.yaml");
        if !file.exists() {
            continue;
        }
        let name = app.file_name().unwrap().to_str().unwrap().to_string();
        let Some(paths) = load(&file)["paths"].as_mapping().cloned() else {
            continue;
        };
        for (_, item) in paths {
            for (_, op) in item.as_mapping().into_iter().flatten() {
                if let Some(id) = op["operationId"].as_str() {
                    out.insert(format!("{name}.{id}"));
                }
            }
        }
    }
    out
}

/// stories/ の UserStory（ファイル名の拡張子を除いたもの、中身）。
fn stories() -> Vec<(String, Yaml)> {
    let dir = specs_dir().join("stories");
    let mut out: Vec<(String, Yaml)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "yaml"))
        .map(|path| {
            let stem = path.file_stem().unwrap().to_str().unwrap().to_string();
            (stem, load(&path))
        })
        .filter(|(_, story)| story["type"].as_str() == Some("UserStory"))
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn ストーリーが_3_本以上ある() {
    // factory-engine の roadmap.yaml の user_story（P5a）。既存の機能に遡って書いた最初の 3 本
    assert!(
        stories().len() >= 3,
        "specs/stories/ の UserStory が 3 本より少ない"
    );
}

#[test]
fn 必須の欄があり_id_とファイル名が揃う() {
    for (stem, story) in stories() {
        for key in REQUIRED {
            let value = &story[key];
            let empty = value.is_null()
                || value.as_str().is_some_and(str::is_empty)
                || value.as_sequence().is_some_and(Vec::is_empty);
            assert!(!empty, "{stem}: {key} がない・空");
        }
        assert_eq!(
            str_of(&story["id"], "id"),
            stem,
            "{stem}: id とファイル名が違う"
        );
        for (i, step) in seq(&story["steps"], "steps").iter().enumerate() {
            for key in ["where", "do", "sees"] {
                str_of(&step[key], &format!("{stem}: steps[{i}].{key}"));
            }
        }
        if let Some(status) = story["status"].as_str() {
            assert!(
                STATUSES.contains(&status),
                "{stem}: status {status} は {STATUSES:?} のどれでもない"
            );
        }
    }
}

#[test]
fn 利用者の種類が_actors_yaml_にある() {
    let actors = actors();
    for (stem, story) in stories() {
        let mut used = vec![str_of(&story["actor"], "actor").to_string()];
        used.extend(
            seq(&story["reader"], "reader")
                .iter()
                .map(|r| str_of(r, "reader").to_string()),
        );
        for step in seq(&story["steps"], "steps") {
            if let Some(by) = step["by"].as_str() {
                used.push(by.to_string());
            }
        }
        for actor in used {
            assert!(
                actors.contains(&actor),
                "{stem}: {actor} が stories/actors.yaml にない"
            );
        }
    }
}

#[test]
fn known_gaps_の歩みが_steps_か_exit_を指す() {
    for (stem, story) in stories() {
        let steps = seq(&story["steps"], "steps").len();
        for gap in story["known_gaps"].as_sequence().into_iter().flatten() {
            str_of(&gap["note"], &format!("{stem}: known_gaps.note"));
            match &gap["step"] {
                Yaml::Number(n) => {
                    let n = n.as_u64().unwrap_or(0) as usize;
                    assert!(
                        (1..=steps).contains(&n),
                        "{stem}: known_gaps.step {n} が steps の範囲（1〜{steps}）にない"
                    );
                }
                other => assert_eq!(
                    other.as_str(),
                    Some("exit"),
                    "{stem}: known_gaps.step は steps の何歩目か exit"
                ),
            }
        }
    }
}

#[test]
fn touches_の_id_が正本にある() {
    let contexts = ids(
        &spec("context-map.yaml")["contexts"],
        "id",
        "context-map.yaml の contexts",
    );
    let terms = ids(
        &spec("glossary.yaml")["terms"],
        "id",
        "glossary.yaml の terms",
    );
    let events = ids(
        &spec("analytics/events.yaml")["events"],
        "name",
        "events.yaml の events",
    );
    let operations = operations();
    let apps: BTreeSet<String> = operations
        .iter()
        .map(|op| op.split('.').next().unwrap().to_string())
        .collect();
    for (stem, story) in stories() {
        let touches = &story["touches"];
        let checks: [(&str, &BTreeSet<String>, &str); 5] = [
            ("apps", &apps, "specs/contracts/api/<app>/openapi.yaml"),
            ("contexts", &contexts, "context-map.yaml"),
            ("terms", &terms, "glossary.yaml"),
            (
                "operations",
                &operations,
                "specs/contracts/api/<app>/openapi.yaml の operationId",
            ),
            ("events", &events, "analytics/events.yaml"),
        ];
        for (key, known, source) in checks {
            for value in touches[key].as_sequence().into_iter().flatten() {
                let value = str_of(value, &format!("{stem}: touches.{key}"));
                assert!(
                    known.contains(value),
                    "{stem}: touches.{key} の {value} が {source} にない"
                );
            }
        }
    }
}
