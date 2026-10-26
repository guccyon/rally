//! 発火の網羅: events.yaml（specs/analytics/events.yaml）のすべての行動イベントが、どこかのユースケースから発火されるか。
//!
//! ソースを静的に読む。下の対応表（イベント名 → 変種 → 発火するユースケースとファイル）を正とし、
//! (1) 表のイベント名が events.yaml とちょうど一致する、(2) 変種の `name()` が表のイベント名を返す、
//! (3) 表のファイルにユースケースがあり、本体（`#[cfg(test)]` より前）でその変種を `emit` している、
//! (4) 表にない `emit` がない、(5) ファイルのモジュールが events.yaml のコンテキストに合う、を確かめる。
//! 発火の中身（属性）は各ユースケースのテストと `crates/interface/tests/` が確かめる。

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde_yaml::Value as Yaml;

/// 発火箇所の対応表: (events.yaml の name, `AnalyticsEvent` の変種, ユースケース, `src/` からのファイル)。
const EMITTERS: &[(&str, &str, &str, &str)] = &[
    (
        "account.registered",
        "AccountRegistered",
        "RegisterAccount",
        "identity/register_account.rs",
    ),
    (
        "slot.published",
        "SlotPublished",
        "PublishSlot",
        "scheduling/publish_slot.rs",
    ),
    (
        "booking.created",
        "BookingCreated",
        "CreateBooking",
        "scheduling/create_booking.rs",
    ),
    (
        "booking.confirmed",
        "BookingConfirmed",
        "ConfirmBooking",
        "scheduling/confirm_booking.rs",
    ),
    (
        "booking.cancelled",
        "BookingCancelled",
        "CancelBooking",
        "scheduling/cancel_booking.rs",
    ),
    (
        "call.started",
        "CallStarted",
        "StartLiveLessonSession",
        "live_lesson/start_live_lesson_session.rs",
    ),
    (
        "call.ended",
        "CallEnded",
        "EndLiveLessonSession",
        "live_lesson/end_live_lesson_session.rs",
    ),
    (
        "video.submitted",
        "VideoSubmitted",
        "SubmitVideo",
        "async_lesson/submit_video.rs",
    ),
    (
        "video_review.returned",
        "VideoReviewReturned",
        "ReturnVideoReview",
        "async_lesson/return_video_review.rs",
    ),
    (
        "lesson.completed",
        "LessonCompleted",
        "RecordLessonCompletion",
        "lesson/record_lesson_completion.rs",
    ),
    (
        "subscription.started",
        "SubscriptionStarted",
        "StartSubscription",
        "entitlement/start_subscription.rs",
    ),
];

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn events_yaml() -> Vec<Yaml> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../specs/analytics/events.yaml"
    );
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let root: Yaml = serde_yaml::from_str(&text).unwrap();
    root["events"].as_sequence().unwrap().clone()
}

/// 空白（改行を含む）を除いたテキスト。rustfmt の折り返しに左右されずに探すため。
fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// テストを除いた本体（最初の `#[cfg(test)]` より前）。
fn production(text: &str) -> &str {
    text.split("#[cfg(test)]").next().unwrap()
}

fn rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, files);
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path);
        }
    }
}

#[test]
fn every_event_of_events_yaml_has_exactly_one_row_in_the_table() {
    let defined: Vec<String> = events_yaml()
        .iter()
        .map(|e| e["name"].as_str().unwrap().to_string())
        .collect();
    let table: Vec<String> = EMITTERS.iter().map(|row| row.0.to_string()).collect();
    let unique: BTreeSet<&String> = table.iter().collect();
    assert_eq!(unique.len(), table.len(), "表のイベント名が重複している");
    assert_eq!(
        unique,
        defined.iter().collect::<BTreeSet<_>>(),
        "表と events.yaml のイベント名が一致しない"
    );
}

#[test]
fn variants_return_the_names_of_the_table() {
    let analytics = compact(&fs::read_to_string(src().join("analytics.rs")).unwrap());
    for (name, variant, _, _) in EMITTERS {
        assert!(
            analytics.contains(&format!("AnalyticsEvent::{variant}{{..}}=>\"{name}\"")),
            "AnalyticsEvent::{variant} の name() が {name} でない"
        );
    }
}

#[test]
fn every_event_is_emitted_by_its_use_case() {
    for (name, variant, use_case, file) in EMITTERS {
        let text = fs::read_to_string(src().join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        let body = compact(production(&text));
        assert!(
            body.contains(&format!("pubstruct{use_case}<")),
            "{file} にユースケース {use_case} がない"
        );
        assert!(
            body.contains(&format!(".emit(AnalyticsEvent::{variant}{{")),
            "{file} の本体が {name}（AnalyticsEvent::{variant}）を発火していない"
        );
    }
}

#[test]
fn no_emission_is_missing_from_the_table() {
    let mut files = Vec::new();
    rust_files(&src(), &mut files);
    let table: BTreeSet<(String, String)> = EMITTERS
        .iter()
        .map(|(_, variant, _, file)| (variant.to_string(), file.to_string()))
        .collect();
    let marker = ".emit(AnalyticsEvent::";
    for path in files {
        let file = path
            .strip_prefix(src())
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let body = compact(production(&fs::read_to_string(&path).unwrap()));
        for (index, _) in body.match_indices(marker) {
            let variant: String = body[index + marker.len()..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            assert!(
                table.contains(&(variant.clone(), file.clone())),
                "{file} が AnalyticsEvent::{variant} を発火しているが、表にない"
            );
        }
    }
}

/// 発火するファイルのモジュールは events.yaml の context（contexts）に合う。`lesson` はコンテキストではなく、
/// live-lesson と async-lesson が共有する完了記録のモジュール（docs/adr/0006）。
#[test]
fn emitting_modules_belong_to_the_contexts_of_events_yaml() {
    let events = events_yaml();
    for (name, _, _, file) in EMITTERS {
        let event = events
            .iter()
            .find(|e| e["name"].as_str() == Some(name))
            .unwrap();
        let contexts: BTreeSet<&str> = match event["contexts"].as_sequence() {
            Some(list) => list.iter().map(|c| c.as_str().unwrap()).collect(),
            None => BTreeSet::from([event["context"].as_str().unwrap()]),
        };
        let module = file.split('/').next().unwrap();
        let module_contexts: BTreeSet<String> = match module {
            "lesson" => BTreeSet::from(["live-lesson".to_string(), "async-lesson".to_string()]),
            other => BTreeSet::from([other.replace('_', "-")]),
        };
        assert_eq!(
            module_contexts,
            contexts.iter().map(|c| c.to_string()).collect(),
            "{name} を発火する {file} のモジュールが events.yaml のコンテキストと合わない"
        );
    }
}
