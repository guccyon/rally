//! セマンティック層（specs/analytics/kpi.yaml）の検査。KPI の式が events.yaml にある名前だけを参照しているか。
//!
//! 式の書き方は kpi.yaml の冒頭のコメント。ここでは式を評価せず、名前だけを読む。

use std::collections::{BTreeMap, BTreeSet};

use serde_yaml::Value as Yaml;

fn spec(relative: &str) -> Yaml {
    let path = format!(
        "{}/../../../../specs/{relative}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn strings(node: &Yaml) -> Vec<String> {
    node.as_sequence()
        .unwrap_or_else(|| panic!("配列でない: {node:?}"))
        .iter()
        .map(|v| {
            v.as_str()
                .unwrap_or_else(|| panic!("文字列でない（YAML の true などは引用符で囲む）: {v:?}"))
                .to_string()
        })
        .collect()
}

/// events.yaml の name から required の属性。
fn events() -> BTreeMap<String, BTreeSet<String>> {
    spec("analytics/events.yaml")["events"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|event| {
            (
                event["name"].as_str().unwrap().to_string(),
                strings(&event["required"]).into_iter().collect(),
            )
        })
        .collect()
}

fn kpis() -> Vec<Yaml> {
    spec("analytics/kpi.yaml")["kpis"]
        .as_sequence()
        .expect("kpi.yaml に kpis がない")
        .clone()
}

fn text<'a>(kpi: &'a Yaml, key: &str) -> &'a str {
    kpi[key]
        .as_str()
        .unwrap_or_else(|| panic!("{:?} に文字列の {key} がない", kpi["id"]))
}

/// 式の中の名前（英字か `_` で始まり、英数字・`_`・`.` が続く）。数・演算子・括弧は読み飛ばす。
fn names(formula: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut chars = formula.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphabetic() || c == '_' {
            let mut name = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
                    name.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            names.push(name);
        } else if c.is_ascii_digit() {
            while chars
                .peek()
                .is_some_and(|c| c.is_ascii_digit() || *c == '.')
            {
                chars.next();
            }
        } else {
            chars.next();
        }
    }
    names
}

/// 式が参照しているもの。
#[derive(Debug, Default)]
struct References {
    /// 参照したイベント名
    events: BTreeSet<String>,
    /// 参照した属性（イベント名, 属性）
    attributes: BTreeSet<(String, String)>,
    /// 参照した他の KPI の id
    kpis: BTreeSet<String>,
    /// 別名からイベント名
    aliases: BTreeMap<String, String>,
}

/// `name`（`<イベント>.<属性>`・`<別名>.<属性>`・イベント名）を解決する。解決できなければ失敗の理由。
fn resolve(
    name: &str,
    events: &BTreeMap<String, BTreeSet<String>>,
    aliases: &BTreeMap<String, String>,
) -> Result<(String, Option<String>), String> {
    let parts: Vec<&str> = name.split('.').collect();
    let attribute = |event: &str, attribute: &str| {
        if events[event].contains(attribute) {
            Ok((event.to_string(), Some(attribute.to_string())))
        } else {
            Err(format!("{name}: {event} の required に {attribute} がない"))
        }
    };
    match parts.as_slice() {
        [alias, attr] if aliases.contains_key(*alias) => attribute(&aliases[*alias], attr),
        [_, _] if events.contains_key(name) => Ok((name.to_string(), None)),
        [object, action, attr] => {
            let event = format!("{object}.{action}");
            if events.contains_key(&event) {
                attribute(&event, attr)
            } else {
                Err(format!("{name}: events.yaml に {event} がない"))
            }
        }
        _ => Err(format!(
            "{name}: events.yaml のイベント名・<イベント>.<属性>・<別名>.<属性> のどれでもない"
        )),
    }
}

/// `formula` の名前を検査して、参照しているものを返す。
fn references(
    id: &str,
    formula: &str,
    vocabulary: &BTreeSet<String>,
    events: &BTreeMap<String, BTreeSet<String>>,
    kpi_ids: &BTreeSet<String>,
) -> References {
    let names = names(formula);
    let mut refs = References::default();
    // `<イベント> as <別名>`
    for window in names.windows(3) {
        if window[1] == "as" {
            assert!(
                events.contains_key(&window[0]),
                "{id}: `{} as {}` の {} が events.yaml のイベント名でない",
                window[0],
                window[2],
                window[0]
            );
            let alias = &window[2];
            assert!(
                !alias.contains('.')
                    && !vocabulary.contains(alias)
                    && !kpi_ids.contains(alias)
                    && !events.contains_key(alias),
                "{id}: 別名 {alias} が語彙・KPI の id・イベント名と紛らわしい"
            );
            if let Some(previous) = refs.aliases.insert(alias.clone(), window[0].clone()) {
                assert_eq!(
                    previous, window[0],
                    "{id}: 別名 {alias} が別のイベントに使われている"
                );
            }
        }
    }
    for name in &names {
        if vocabulary.contains(name) || refs.aliases.contains_key(name) {
            continue;
        }
        if kpi_ids.contains(name) {
            assert_ne!(name, id, "{id}: 自分自身を参照している");
            refs.kpis.insert(name.clone());
            continue;
        }
        let (event, attribute) =
            resolve(name, events, &refs.aliases).unwrap_or_else(|e| panic!("{id}: {e}"));
        refs.events.insert(event.clone());
        if let Some(attribute) = attribute {
            refs.attributes.insert((event, attribute));
        }
    }
    refs.events.extend(refs.aliases.values().cloned());
    refs
}

struct Checked {
    id: String,
    inputs: BTreeSet<String>,
    refs: References,
}

/// すべての KPI の式を検査する（他の KPI の inputs を含めて比べるのは [`kpi_inputs_are_exactly_the_events_the_formula_reads`]）。
fn checked() -> Vec<Checked> {
    let root = spec("analytics/kpi.yaml");
    let vocabulary: BTreeSet<String> = strings(&root["vocabulary"]).into_iter().collect();
    let events = events();
    let kpis = kpis();
    let kpi_ids: BTreeSet<String> = kpis.iter().map(|k| text(k, "id").to_string()).collect();
    kpis.iter()
        .map(|kpi| {
            let id = text(kpi, "id");
            Checked {
                id: id.to_string(),
                inputs: strings(&kpi["inputs"]).into_iter().collect(),
                refs: references(id, text(kpi, "formula"), &vocabulary, &events, &kpi_ids),
            }
        })
        .collect()
}

#[test]
fn kpi_yaml_defines_the_six_kpis_of_the_issue_with_one_north_star() {
    let kpis = kpis();
    let ja: Vec<&str> = kpis.iter().map(|k| text(k, "ja")).collect();
    assert_eq!(
        ja.iter().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "初回レッスン後の 1 ヶ月継続率",
            "コーチ別継続率の分散",
            "コーチの予約枠充足率",
            "通話品質スコア",
            "添削の返却リードタイム",
            "初回レッスンまでの日数",
        ])
    );
    assert_eq!(ja.len(), 6, "ja が重複している");

    let ids: BTreeSet<&str> = kpis.iter().map(|k| text(k, "id")).collect();
    assert_eq!(ids.len(), kpis.len(), "id が重複している");
    for id in &ids {
        assert!(
            id.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            "{id} が英小文字スネークケースでない"
        );
    }

    let north_stars: Vec<&str> = kpis
        .iter()
        .filter(|k| {
            k["north_star"]
                .as_bool()
                .expect("north_star は true / false")
        })
        .map(|k| text(k, "ja"))
        .collect();
    assert_eq!(north_stars, ["初回レッスン後の 1 ヶ月継続率"]);
}

#[test]
fn kpis_have_a_window_a_window_attribute_and_a_direction() {
    let events = events();
    for (kpi, checked) in kpis().iter().zip(checked()) {
        let id = &checked.id;
        let window = text(kpi, "window");
        let days = window
            .strip_suffix('d')
            .unwrap_or_else(|| panic!("{id}: window は <日数>d: {window}"));
        assert!(
            days.parse::<u32>().is_ok_and(|d| d > 0),
            "{id}: window の日数: {window}"
        );
        assert!(
            ["up_is_good", "down_is_good"].contains(&text(kpi, "direction")),
            "{id}: direction は up_is_good か down_is_good"
        );

        // window を当てる時刻は、母集団（`from <イベント> as <別名>`）の時刻の属性
        let formula = text(kpi, "formula");
        let from = names(formula);
        assert_eq!(from[0], "from", "{id}: 式は from で始める");
        assert_eq!(from[2], "as", "{id}: from <イベント> as <別名>");
        let window_on = text(kpi, "window_on");
        let (alias, attribute) = window_on
            .split_once('.')
            .unwrap_or_else(|| panic!("{id}: window_on は <別名>.<属性>: {window_on}"));
        assert_eq!(alias, from[3], "{id}: window_on は母集団の別名で書く");
        assert!(
            attribute.ends_with("_at") && events[&from[1]].contains(attribute),
            "{id}: window_on の {attribute} が {} の時刻の属性でない",
            from[1]
        );
    }
}

#[test]
fn formulas_refer_only_to_names_in_events_yaml() {
    // 検査そのものは checked() の中（events.yaml にない名前があれば panic する）
    let events = events();
    for checked in checked() {
        assert!(
            !checked.refs.events.is_empty(),
            "{}: 式がイベントを参照していない",
            checked.id
        );
        for event in &checked.refs.events {
            assert!(events.contains_key(event), "{}: {event}", checked.id);
        }
    }
}

#[test]
fn kpi_inputs_are_exactly_the_events_the_formula_reads() {
    let checked = checked();
    let by_id: BTreeMap<&str, &Checked> = checked.iter().map(|c| (c.id.as_str(), c)).collect();
    for kpi in &checked {
        let mut read = kpi.refs.events.clone();
        for other in &kpi.refs.kpis {
            read.extend(by_id[other.as_str()].inputs.iter().cloned());
            assert!(
                by_id[other.as_str()].refs.kpis.is_empty(),
                "{}: 参照先の {other} がさらに KPI を参照している（1 段まで）",
                kpi.id
            );
        }
        assert_eq!(
            kpi.inputs, read,
            "{}: inputs が式の参照と一致しない",
            kpi.id
        );
    }
}

#[test]
fn vocabulary_does_not_shadow_events_attributes_or_kpis() {
    let vocabulary = strings(&spec("analytics/kpi.yaml")["vocabulary"]);
    let events = events();
    let ids: BTreeSet<String> = kpis().iter().map(|k| text(k, "id").to_string()).collect();
    for word in &vocabulary {
        assert!(!word.contains('.'), "{word}");
        assert!(!ids.contains(word), "語彙 {word} が KPI の id と同じ");
        for (event, required) in &events {
            assert!(
                !required.contains(word),
                "語彙 {word} が {event} の属性と同じ"
            );
        }
    }
}

/// events.yaml の `kpi` は kpi.yaml の `ja` を指し、そのイベントはその KPI の inputs に入っている。
#[test]
fn kpi_of_events_yaml_points_to_a_kpi_that_reads_the_event() {
    let kpis = kpis();
    for event in spec("analytics/events.yaml")["events"]
        .as_sequence()
        .unwrap()
    {
        let Some(ja) = event["kpi"].as_str() else {
            continue;
        };
        let name = event["name"].as_str().unwrap();
        let kpi = kpis
            .iter()
            .find(|k| text(k, "ja") == ja)
            .unwrap_or_else(|| panic!("{name} の kpi「{ja}」が kpi.yaml にない"));
        assert!(
            strings(&kpi["inputs"]).iter().any(|i| i == name),
            "{name} が「{ja}」の inputs にない"
        );
    }
}

/// 通話品質スコアは docs/adr/0002 Q1 のとおり `call.ended` の 4 属性から計算する。
#[test]
fn call_quality_score_reads_the_four_attributes_of_adr_0002() {
    let checked = checked();
    let score = checked
        .iter()
        .find(|c| c.id == "call_quality_score")
        .unwrap();
    let attributes: BTreeSet<(&str, &str)> = score
        .refs
        .attributes
        .iter()
        .map(|(e, a)| (e.as_str(), a.as_str()))
        .collect();
    assert_eq!(
        attributes,
        BTreeSet::from([
            ("call.ended", "packet_loss_rate"),
            ("call.ended", "jitter_ms"),
            ("call.ended", "round_trip_ms"),
            ("call.ended", "disconnect_count"),
        ])
    );
}

/// 検査自体が名前の誤りを拾うこと。
#[test]
fn unknown_names_are_rejected() {
    let vocabulary: BTreeSet<String> = strings(&spec("analytics/kpi.yaml")["vocabulary"])
        .into_iter()
        .collect();
    let events = events();
    let ids = BTreeSet::new();
    for formula in [
        "from call.ended as e avg(e.mos)",
        "from call.finished as e avg(e.jitter_ms)",
        "from call.ended as e avg(call.ended.mos)",
        "from call.ended as e avg(quality_of(e.jitter_ms))",
        "from call.ended as e avg(x.jitter_ms)",
        "from call.ended as from avg(from.jitter_ms)",
    ] {
        let result = std::panic::catch_unwind(|| {
            references("test", formula, &vocabulary, &events, &ids);
        });
        assert!(result.is_err(), "通ってしまった: {formula}");
    }
    let ok = references(
        "test",
        "from call.ended as e avg(e.jitter_ms + call.ended.round_trip_ms)",
        &vocabulary,
        &events,
        &ids,
    );
    assert_eq!(ok.events, BTreeSet::from(["call.ended".to_string()]));
}
