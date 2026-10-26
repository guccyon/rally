//! 境界間イベントのペイロードが AsyncAPI（specs/contracts/events/asyncapi.yaml）の契約と一致するか。

use std::{
    collections::BTreeSet,
    time::{Duration, SystemTime},
};

use application::{
    domain_event::DomainEvent, identity::account_id_uuid, scheduling::booking_id_uuid,
};
use domain::{identity::AccountId, scheduling::BookingId};
use infra::{LoggingDomainEventPublisher, domain_event_payload::payload};
use serde_json::Value;
use serde_yaml::Value as Yaml;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

fn asyncapi() -> Yaml {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../specs/contracts/events/asyncapi.yaml"
    );
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_yaml::from_str(&text).unwrap()
}

/// `#/components/...` の参照を辿る。
fn resolve<'a>(root: &'a Yaml, node: &'a Yaml) -> &'a Yaml {
    match node.get("$ref").and_then(Yaml::as_str) {
        Some(reference) => {
            let path = reference.strip_prefix("#/").expect("内部参照だけを扱う");
            resolve(root, path.split('/').fold(root, |node, key| &node[key]))
        }
        None => node,
    }
}

/// チャンネル `channel` のメッセージのペイロードのスキーマ。
fn payload_schema<'a>(root: &'a Yaml, channel: &str) -> &'a Yaml {
    let messages = root["channels"][channel]["messages"]
        .as_mapping()
        .unwrap_or_else(|| panic!("AsyncAPI にチャンネル {channel} がない"));
    assert_eq!(messages.len(), 1, "{channel} のメッセージは 1 つ");
    let message = resolve(root, messages.values().next().unwrap());
    assert_eq!(message["name"].as_str(), Some(channel));
    resolve(root, &message["payload"])
}

/// `event` のペイロードの属性が、AsyncAPI のそのチャンネルの `required` とちょうど一致し、
/// 各属性の型（string / boolean）と `format: date-time` を満たすか。
fn assert_payload_matches_asyncapi(event: &DomainEvent) -> Value {
    let root = asyncapi();
    let schema = payload_schema(&root, event.name());

    let body = payload(event).unwrap();
    let object = body.as_object().unwrap();

    let required: BTreeSet<&str> = schema["required"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|k| k.as_str().unwrap())
        .collect();
    let actual: BTreeSet<&str> = object.keys().map(String::as_str).collect();
    assert_eq!(
        actual, required,
        "ペイロードの属性が AsyncAPI の required と一致しない"
    );

    let properties = schema["properties"].as_mapping().unwrap();
    for (key, value) in object {
        let property = resolve(&root, &properties[key.as_str()]);
        match property["type"].as_str() {
            Some("string") => {
                let text = value
                    .as_str()
                    .unwrap_or_else(|| panic!("{key} が string でない: {value}"));
                if property["format"].as_str() == Some("date-time") {
                    assert!(
                        OffsetDateTime::parse(text, &Rfc3339).is_ok(),
                        "{key} が RFC 3339 でない: {text}"
                    );
                }
            }
            Some("boolean") => assert!(value.is_boolean(), "{key} が boolean でない: {value}"),
            other => panic!("{key}: 検査できない type {other:?}"),
        }
    }
    body
}

/// 2026-10-26T00:00:00Z
fn oct26() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
}

fn booking_confirmed() -> DomainEvent {
    DomainEvent::BookingConfirmed {
        event_id: uuid_from(0x0123_4567_89ab_4def_8123_4567_89ab_cdef),
        occurred_at: oct26(),
        student_id: AccountId::from_u128(1),
        coach_id: AccountId::from_u128(2),
        booking_id: BookingId::from_u128(3),
    }
}

fn booking_cancelled(consumes_quota: bool) -> DomainEvent {
    DomainEvent::BookingCancelled {
        event_id: uuid_from(0x0123_4567_89ab_4def_8123_4567_89ab_cdef),
        occurred_at: oct26(),
        student_id: AccountId::from_u128(1),
        coach_id: AccountId::from_u128(2),
        booking_id: BookingId::from_u128(3),
        consumes_quota,
        cancelled_at: oct26(),
    }
}

/// infra は `uuid` クレートに直接依存しないので、sqlx が再公開する `Uuid` で作る（同じ型）。
fn uuid_from(value: u128) -> sqlx::types::Uuid {
    sqlx::types::Uuid::from_u128(value)
}

fn assert_common_attributes(body: &Value) {
    assert_eq!(body["event_id"], "01234567-89ab-4def-8123-456789abcdef");
    assert_eq!(body["occurred_at"], "2026-10-26T00:00:00Z");
    assert_eq!(
        body["student_id"],
        account_id_uuid::format(AccountId::from_u128(1))
    );
    assert_eq!(
        body["coach_id"],
        account_id_uuid::format(AccountId::from_u128(2))
    );
    assert_eq!(
        body["booking_id"],
        booking_id_uuid::format(BookingId::from_u128(3))
    );
}

#[test]
fn booking_confirmed_payload_has_exactly_the_required_attributes_of_the_asyncapi() {
    let event = booking_confirmed();
    assert_eq!(event.name(), "BookingConfirmed");
    let body = assert_payload_matches_asyncapi(&event);
    assert_common_attributes(&body);
}

#[test]
fn booking_cancelled_payload_has_exactly_the_required_attributes_of_the_asyncapi() {
    for consumes_quota in [true, false] {
        let event = booking_cancelled(consumes_quota);
        assert_eq!(event.name(), "BookingCancelled");
        let body = assert_payload_matches_asyncapi(&event);
        assert_common_attributes(&body);
        assert_eq!(body["consumes_quota"], consumes_quota);
        assert_eq!(body["cancelled_at"], "2026-10-26T00:00:00Z");
    }
}

#[test]
fn booking_cancelled_is_subscribed_by_entitlement() {
    let root = asyncapi();
    let channel = &root["channels"]["BookingCancelled"];
    let names = |key: &str| -> Vec<String> {
        channel[key]
            .as_sequence()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(names("x-publisher"), ["scheduling"]);
    assert_eq!(
        names("x-subscribers"),
        ["entitlement", "notification", "analytics"]
    );
}

#[test]
fn logging_publisher_writes_the_message_name_and_the_payload_on_one_line() {
    for event in [booking_confirmed(), booking_cancelled(true)] {
        let line = LoggingDomainEventPublisher::line(&event).unwrap();
        let json = line
            .strip_prefix(&format!("domain_event {} ", event.name()))
            .unwrap_or_else(|| panic!("{line}"));
        assert!(!json.contains('\n'));
        assert_eq!(
            serde_json::from_str::<Value>(json).unwrap(),
            payload(&event).unwrap()
        );
    }
}
