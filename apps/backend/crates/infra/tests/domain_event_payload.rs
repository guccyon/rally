//! 境界間イベントのペイロードが AsyncAPI（specs/contracts/events/asyncapi.yaml）の契約と一致するか。

use std::{
    collections::BTreeSet,
    time::{Duration, SystemTime},
};

use application::{
    async_lesson::{video_id_uuid, video_review_id_uuid},
    domain_event::DomainEvent,
    entitlement::subscription_id_uuid,
    identity::account_id_uuid,
    lesson::lesson_completion_id_uuid,
    messaging::{message_id_uuid, message_thread_id_uuid},
    scheduling::booking_id_uuid,
};
use domain::{
    async_lesson::{VideoId, VideoReviewId},
    entitlement::SubscriptionId,
    identity::AccountId,
    lesson::LessonCompletionId,
    messaging::{MessageId, MessageSender, MessageThreadId},
    scheduling::BookingId,
};
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

/// AsyncAPI のチャンネル `channel` の `key`（x-publisher / x-subscribers）。
fn contexts(channel: &str, key: &str) -> Vec<String> {
    asyncapi()["channels"][channel][key]
        .as_sequence()
        .unwrap_or_else(|| panic!("{channel} に {key} がない"))
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn booking_events_are_subscribed_by_entitlement() {
    // 予約の確定は messaging も購読する（生徒と担当コーチのスレッドを開く）
    for (channel, subscribers) in [
        (
            "BookingConfirmed",
            &["entitlement", "messaging", "notification", "analytics"][..],
        ),
        (
            "BookingCancelled",
            &["entitlement", "notification", "analytics"][..],
        ),
    ] {
        assert_eq!(contexts(channel, "x-publisher"), ["scheduling"]);
        assert_eq!(contexts(channel, "x-subscribers"), subscribers, "{channel}");
    }
}

fn subscription_started() -> DomainEvent {
    DomainEvent::SubscriptionStarted {
        event_id: uuid_from(0x0123_4567_89ab_4def_8123_4567_89ab_cdef),
        occurred_at: oct26(),
        student_id: AccountId::from_u128(1),
        subscription_id: SubscriptionId::from_u128(4),
    }
}

#[test]
fn subscription_started_payload_has_exactly_the_required_attributes_of_the_asyncapi() {
    let event = subscription_started();
    assert_eq!(event.name(), "SubscriptionStarted");
    let body = assert_payload_matches_asyncapi(&event);
    assert_eq!(body["event_id"], "01234567-89ab-4def-8123-456789abcdef");
    assert_eq!(body["occurred_at"], "2026-10-26T00:00:00Z");
    assert_eq!(
        body["student_id"],
        account_id_uuid::format(AccountId::from_u128(1))
    );
    assert_eq!(
        body["subscription_id"],
        subscription_id_uuid::format(SubscriptionId::from_u128(4))
    );
    assert_eq!(
        contexts("SubscriptionStarted", "x-publisher"),
        ["entitlement"]
    );
}

fn lesson_completed() -> DomainEvent {
    DomainEvent::LessonCompleted {
        event_id: uuid_from(0x0123_4567_89ab_4def_8123_4567_89ab_cdef),
        occurred_at: oct26(),
        student_id: AccountId::from_u128(1),
        coach_id: AccountId::from_u128(2),
        lesson_id: LessonCompletionId::from_u128(5),
    }
}

#[test]
fn lesson_completed_payload_has_exactly_the_required_attributes_of_the_asyncapi() {
    let event = lesson_completed();
    assert_eq!(event.name(), "LessonCompleted");
    let body = assert_payload_matches_asyncapi(&event);
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
        body["lesson_id"],
        lesson_completion_id_uuid::format(LessonCompletionId::from_u128(5))
    );
    // 完了記録は live-lesson・async-lesson のそれぞれの集約が発行する（docs/adr/0006）
    assert_eq!(
        contexts("LessonCompleted", "x-publisher"),
        ["live-lesson", "async-lesson"]
    );
    assert_eq!(
        contexts("LessonCompleted", "x-subscribers"),
        ["billing", "review", "analytics"]
    );
}

fn video_review_returned() -> DomainEvent {
    DomainEvent::VideoReviewReturned {
        event_id: uuid_from(0x0123_4567_89ab_4def_8123_4567_89ab_cdef),
        occurred_at: oct26(),
        student_id: AccountId::from_u128(1),
        coach_id: AccountId::from_u128(2),
        video_id: VideoId::from_u128(6),
        review_id: VideoReviewId::from_u128(7),
    }
}

#[test]
fn video_review_returned_payload_has_exactly_the_required_attributes_of_the_asyncapi() {
    let event = video_review_returned();
    assert_eq!(event.name(), "VideoReviewReturned");
    let body = assert_payload_matches_asyncapi(&event);
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
        body["video_id"],
        video_id_uuid::format(VideoId::from_u128(6))
    );
    assert_eq!(
        body["review_id"],
        video_review_id_uuid::format(VideoReviewId::from_u128(7))
    );
    assert_eq!(
        contexts("VideoReviewReturned", "x-publisher"),
        ["async-lesson"]
    );
    assert_eq!(
        contexts("VideoReviewReturned", "x-subscribers"),
        ["notification", "messaging"]
    );
}

fn video_review_requested() -> DomainEvent {
    DomainEvent::VideoReviewRequested {
        event_id: uuid_from(0x0123_4567_89ab_4def_8123_4567_89ab_cdef),
        occurred_at: oct26(),
        student_id: AccountId::from_u128(1),
        coach_id: AccountId::from_u128(2),
        video_id: VideoId::from_u128(6),
    }
}

#[test]
fn video_review_requested_payload_has_exactly_the_required_attributes_of_the_asyncapi() {
    let event = video_review_requested();
    assert_eq!(event.name(), "VideoReviewRequested");
    let body = assert_payload_matches_asyncapi(&event);
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
        body["video_id"],
        video_id_uuid::format(VideoId::from_u128(6))
    );
    assert_eq!(
        contexts("VideoReviewRequested", "x-publisher"),
        ["async-lesson"]
    );
    assert_eq!(
        contexts("VideoReviewRequested", "x-subscribers"),
        ["notification", "messaging"]
    );
}

fn message_sent(sender_role: MessageSender) -> DomainEvent {
    DomainEvent::MessageSent {
        event_id: uuid_from(0x0123_4567_89ab_4def_8123_4567_89ab_cdef),
        occurred_at: oct26(),
        student_id: AccountId::from_u128(1),
        coach_id: AccountId::from_u128(2),
        thread_id: MessageThreadId::from_u128(8),
        message_id: MessageId::from_u128(9),
        sender_role,
    }
}

#[test]
fn message_sent_payload_has_exactly_the_required_attributes_of_the_asyncapi() {
    let root = asyncapi();
    let allowed: Vec<&str> =
        payload_schema(&root, "MessageSent")["properties"]["sender_role"]["enum"]
            .as_sequence()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
    for (sender_role, expected) in [
        (MessageSender::Student, "student"),
        (MessageSender::Coach, "coach"),
    ] {
        let event = message_sent(sender_role);
        assert_eq!(event.name(), "MessageSent");
        let body = assert_payload_matches_asyncapi(&event);
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
            body["thread_id"],
            message_thread_id_uuid::format(MessageThreadId::from_u128(8))
        );
        assert_eq!(
            body["message_id"],
            message_id_uuid::format(MessageId::from_u128(9))
        );
        assert_eq!(body["sender_role"], expected);
        assert!(allowed.contains(&expected), "{expected} が enum にない");
    }
    assert_eq!(contexts("MessageSent", "x-publisher"), ["messaging"]);
    assert_eq!(contexts("MessageSent", "x-subscribers"), ["notification"]);
}

#[test]
fn logging_publisher_writes_the_message_name_and_the_payload_on_one_line() {
    for event in [
        booking_confirmed(),
        booking_cancelled(true),
        subscription_started(),
        lesson_completed(),
        video_review_requested(),
        video_review_returned(),
        message_sent(MessageSender::Coach),
    ] {
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
