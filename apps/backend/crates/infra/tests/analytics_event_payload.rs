//! 行動イベントの属性が events.yaml（specs/analytics/events.yaml）の定義と一致するか。

use std::{
    collections::BTreeSet,
    time::{Duration, SystemTime},
};

use application::analytics::AnalyticsEvent;
use domain::{
    async_lesson::{VideoId, VideoReviewId},
    catalog::Sport,
    entitlement::SubscriptionId,
    identity::AccountId,
    lesson::{LessonCompletionId, LessonType},
    live_lesson::SessionId,
    messaging::{MessageId, MessageSender, MessageThreadId},
    review::{LessonReviewId, Rating},
    scheduling::{BookingId, SlotId},
};
use infra::{
    AnalyticsCommonAttributes,
    analytics_event_payload::{attributes, lesson_type_value, line},
};
use serde_json::Value;
use serde_yaml::Value as Yaml;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

fn events_yaml() -> Yaml {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../specs/analytics/events.yaml"
    );
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_yaml::from_str(&text).unwrap()
}

/// events.yaml の `name` の `required`。
fn required(root: &Yaml, name: &str) -> BTreeSet<String> {
    root["events"]
        .as_sequence()
        .unwrap()
        .iter()
        .find(|event| event["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("events.yaml に {name} がない"))["required"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|k| k.as_str().unwrap().to_string())
        .collect()
}

/// 2026-10-26T00:00:00Z
fn oct26() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
}

fn student() -> AccountId {
    AccountId::from_u128(1)
}

fn coach() -> AccountId {
    AccountId::from_u128(2)
}

/// すべての変種を 1 つずつ。変種が増えたら [`exhaustive`] がコンパイルで落ちるので、ここに足す。
fn samples() -> Vec<AnalyticsEvent> {
    let booking_id = BookingId::from_u128(3);
    let slot_id = SlotId::from_u128(4);
    let session_id = SessionId::from_u128(5);
    let video_id = VideoId::from_u128(6);
    vec![
        AnalyticsEvent::AccountRegistered {
            student_id: student(),
            sport: Sport::Tennis,
            occurred_at: oct26(),
        },
        AnalyticsEvent::SlotPublished {
            coach_id: coach(),
            slot_id,
            starts_at: oct26() + Duration::from_secs(86_400),
            occurred_at: oct26(),
        },
        AnalyticsEvent::SlotWithdrawn {
            coach_id: coach(),
            slot_id,
            starts_at: oct26() + Duration::from_secs(86_400),
            occurred_at: oct26(),
        },
        AnalyticsEvent::BookingCreated {
            student_id: student(),
            coach_id: coach(),
            booking_id,
            slot_id,
            is_trial: true,
            occurred_at: oct26(),
        },
        AnalyticsEvent::BookingConfirmed {
            student_id: student(),
            coach_id: coach(),
            booking_id,
            slot_id,
            occurred_at: oct26(),
        },
        AnalyticsEvent::BookingCancelled {
            student_id: student(),
            coach_id: coach(),
            booking_id,
            slot_id,
            hours_before_start: -2,
            occurred_at: oct26(),
        },
        AnalyticsEvent::CallStarted {
            student_id: student(),
            coach_id: coach(),
            booking_id,
            session_id,
            occurred_at: oct26(),
        },
        AnalyticsEvent::CallEnded {
            student_id: student(),
            coach_id: coach(),
            booking_id,
            session_id,
            duration_sec: 1800,
            packet_loss_rate: 2.5,
            jitter_ms: 40,
            round_trip_ms: 210,
            disconnect_count: 1,
            occurred_at: oct26(),
        },
        AnalyticsEvent::VideoSubmitted {
            student_id: student(),
            coach_id: coach(),
            video_id,
            sport: Sport::Soccer,
            occurred_at: oct26(),
        },
        AnalyticsEvent::VideoReviewReturned {
            student_id: student(),
            coach_id: coach(),
            video_id,
            review_id: VideoReviewId::from_u128(7),
            submitted_at: oct26() - Duration::from_secs(3600),
            occurred_at: oct26(),
        },
        AnalyticsEvent::LessonCompleted {
            student_id: student(),
            coach_id: coach(),
            lesson_id: LessonCompletionId::from_u128(8),
            lesson_type: LessonType::Live,
            is_first_lesson: true,
            occurred_at: oct26(),
        },
        AnalyticsEvent::SubscriptionStarted {
            student_id: student(),
            subscription_id: SubscriptionId::from_u128(9),
            sport: Sport::Baseball,
            occurred_at: oct26(),
        },
        AnalyticsEvent::MessageSent {
            student_id: student(),
            coach_id: coach(),
            thread_id: MessageThreadId::from_u128(10),
            message_id: MessageId::from_u128(11),
            sender_role: MessageSender::Coach,
            occurred_at: oct26(),
        },
        AnalyticsEvent::ReviewSubmitted {
            student_id: student(),
            coach_id: coach(),
            lesson_id: LessonCompletionId::from_u128(8),
            review_id: LessonReviewId::from_u128(10),
            rating: Rating::new(4).unwrap(),
            occurred_at: oct26(),
        },
    ]
}

/// 変種の網羅をコンパイラに確かめさせる（変種が増えたら [`samples`] に足す）。
#[allow(dead_code)]
fn exhaustive(event: &AnalyticsEvent) {
    match event {
        AnalyticsEvent::AccountRegistered { .. }
        | AnalyticsEvent::SlotPublished { .. }
        | AnalyticsEvent::SlotWithdrawn { .. }
        | AnalyticsEvent::BookingCreated { .. }
        | AnalyticsEvent::BookingConfirmed { .. }
        | AnalyticsEvent::BookingCancelled { .. }
        | AnalyticsEvent::CallStarted { .. }
        | AnalyticsEvent::CallEnded { .. }
        | AnalyticsEvent::VideoSubmitted { .. }
        | AnalyticsEvent::VideoReviewReturned { .. }
        | AnalyticsEvent::LessonCompleted { .. }
        | AnalyticsEvent::SubscriptionStarted { .. }
        | AnalyticsEvent::MessageSent { .. }
        | AnalyticsEvent::ReviewSubmitted { .. } => {}
    }
}

#[test]
fn samples_cover_every_event_of_events_yaml() {
    let root = events_yaml();
    let defined: BTreeSet<&str> = root["events"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|event| event["name"].as_str().unwrap())
        .collect();
    let samples = samples();
    let sampled: BTreeSet<&str> = samples.iter().map(AnalyticsEvent::name).collect();
    assert_eq!(sampled, defined);
}

#[test]
fn attributes_are_exactly_the_required_attributes_of_events_yaml() {
    let root = events_yaml();
    for event in samples() {
        let body = attributes(&event).unwrap();
        let actual: BTreeSet<String> = body.as_object().unwrap().keys().cloned().collect();
        assert_eq!(
            actual,
            required(&root, event.name()),
            "{} の属性が events.yaml の required と一致しない",
            event.name()
        );
    }
}

#[test]
fn attributes_have_the_documented_shapes() {
    for event in samples() {
        let body = attributes(&event).unwrap();
        for (key, value) in body.as_object().unwrap() {
            let name = event.name();
            if key.ends_with("_at") {
                let text = value.as_str().unwrap();
                let at = OffsetDateTime::parse(text, &Rfc3339)
                    .unwrap_or_else(|e| panic!("{name}.{key} が RFC 3339 でない: {text}: {e}"));
                assert_eq!(at.offset().whole_seconds(), 0, "{name}.{key} は UTC");
            } else if key.ends_with("_id") {
                let text = value.as_str().unwrap();
                assert!(
                    uuid::Uuid::try_parse(text).is_ok() && text.len() == 36,
                    "{name}.{key} が UUID のハイフン区切りでない: {text}"
                );
            } else if key.starts_with("is_") {
                assert!(
                    value.is_boolean(),
                    "{name}.{key} が boolean でない: {value}"
                );
            } else if key == "sport" || key == "lesson_type" || key == "sender_role" {
                assert!(value.is_string(), "{name}.{key} が string でない: {value}");
            } else {
                assert!(value.is_number(), "{name}.{key} が数でない: {value}");
            }
        }
    }
}

#[test]
fn values_follow_the_comments_of_events_yaml() {
    let samples = samples();
    let by_name =
        |name: &str| attributes(samples.iter().find(|e| e.name() == name).unwrap()).unwrap();

    assert_eq!(by_name("account.registered")["sport"], "tennis");
    assert_eq!(
        by_name("account.registered")["occurred_at"],
        "2026-10-26T00:00:00Z"
    );
    assert_eq!(
        by_name("slot.published")["starts_at"],
        "2026-10-27T00:00:00Z"
    );
    assert_eq!(
        by_name("slot.withdrawn")["starts_at"],
        "2026-10-27T00:00:00Z"
    );
    assert_eq!(by_name("booking.cancelled")["hours_before_start"], -2);
    let call = by_name("call.ended");
    assert_eq!(call["packet_loss_rate"], 2.5);
    assert_eq!(call["jitter_ms"], 40);
    assert_eq!(call["round_trip_ms"], 210);
    assert_eq!(call["disconnect_count"], 1);
    assert_eq!(call["duration_sec"], 1800);
    assert_eq!(
        by_name("video_review.returned")["submitted_at"],
        "2026-10-25T23:00:00Z"
    );
    let lesson = by_name("lesson.completed");
    assert_eq!(lesson["lesson_type"], "live_lesson");
    assert_eq!(lesson["is_first_lesson"], true);
    assert_eq!(by_name("message.sent")["sender_role"], "coach");
    assert_eq!(by_name("review.submitted")["rating"], 4);
}

/// events.yaml の注記どおり、student-app の契約の `LessonType` と同じ値。
#[test]
fn lesson_type_values_match_the_student_app_contract() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../specs/contracts/api/student-app/openapi.yaml"
    );
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let root: Yaml = serde_yaml::from_str(&text).unwrap();
    let contract: BTreeSet<&str> = root["components"]["schemas"]["LessonType"]["enum"]
        .as_sequence()
        .expect("契約に LessonType の enum がない")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let values: BTreeSet<&str> = [LessonType::Live, LessonType::Async]
        .into_iter()
        .map(lesson_type_value)
        .collect();
    assert_eq!(values, contract);
}

#[test]
fn line_is_one_json_object_with_the_name_and_the_attributes() {
    for event in samples() {
        let text = line(&event, &AnalyticsCommonAttributes::default()).unwrap();
        assert!(!text.contains('\n'), "{text}");
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["name"], event.name());
        assert_eq!(value["attributes"], attributes(&event).unwrap());
        assert_eq!(value.as_object().unwrap().len(), 2);
    }
}

/// events.yaml の `common_optional` の name。
fn common_optional(root: &Yaml) -> BTreeSet<String> {
    root["common_optional"]
        .as_sequence()
        .expect("events.yaml に common_optional がない")
        .iter()
        .map(|attribute| attribute["name"].as_str().unwrap().to_string())
        .collect()
}

fn known_common() -> AnalyticsCommonAttributes {
    AnalyticsCommonAttributes {
        release_id: Some("guccyon/rally@0.1.0".to_string()),
        app_version: Some("0.1.0".to_string()),
    }
}

/// 共通の任意属性は optional。どのイベントの `required` にも入れない。
#[test]
fn common_optional_attributes_are_not_required_by_any_event() {
    let root = events_yaml();
    let common = common_optional(&root);
    assert_eq!(
        common,
        BTreeSet::from(["release_id".to_string(), "app_version".to_string()])
    );
    for event in samples() {
        let required = required(&root, event.name());
        assert!(
            required.is_disjoint(&common),
            "{} の required に共通の任意属性がある",
            event.name()
        );
    }
}

/// 値が分かる共通の任意属性は、すべてのイベントの行に events.yaml の `common_optional` の名前で付く。
#[test]
fn line_adds_the_known_common_attributes_to_every_event() {
    let root = events_yaml();
    let common = known_common();
    for event in samples() {
        let value: Value = serde_json::from_str(&line(&event, &common).unwrap()).unwrap();
        let actual: BTreeSet<String> = value["attributes"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        let expected: BTreeSet<String> = required(&root, event.name())
            .union(&common_optional(&root))
            .cloned()
            .collect();
        assert_eq!(actual, expected, "{}", event.name());
        assert_eq!(value["attributes"]["release_id"], "guccyon/rally@0.1.0");
        assert_eq!(value["attributes"]["app_version"], "0.1.0");
    }
}

/// 分からない共通の任意属性は付けない（欠けていても行は書ける）。
#[test]
fn line_omits_the_unknown_common_attributes() {
    let only_version = AnalyticsCommonAttributes {
        release_id: None,
        app_version: Some("0.1.0".to_string()),
    };
    for event in samples() {
        let value: Value = serde_json::from_str(&line(&event, &only_version).unwrap()).unwrap();
        let mut expected = attributes(&event).unwrap();
        expected["app_version"] = "0.1.0".into();
        assert_eq!(value["attributes"], expected, "{}", event.name());
    }
}
