//! 行動イベントと、行動トラッキングに送る属性（specs/analytics/events.yaml）の対応表。
//!
//! [`attributes`] は events.yaml の `required` だけ。共通の任意属性（`common_optional`）は [`line`] で値が分かる
//! ときだけ足す。ID は UUID のハイフン区切り、時刻（`occurred_at`・`starts_at`・
//! `submitted_at`）は RFC 3339。`sport` は DB と同じ値（[`crate::identity_values::sport_value`]）。
//! events.yaml との一致は `tests/analytics_event_payload.rs` が確かめる。

use application::{
    analytics::AnalyticsEvent,
    async_lesson::{video_id_uuid, video_review_id_uuid},
    entitlement::subscription_id_uuid,
    identity::account_id_uuid,
    lesson::lesson_id_uuid,
    live_lesson::session_id_uuid,
    review::lesson_review_id_uuid,
    scheduling::{booking_id_uuid, slot_id_uuid},
};
use domain::lesson::LessonType;
use serde_json::{Value, json};

use crate::{
    AnalyticsCommonAttributes, domain_event_payload::date_time, identity_values::sport_value,
};

/// `lesson.completed` の `lesson_type` の値（student-app の契約の `LessonType` と同じ）。
pub const fn lesson_type_value(lesson_type: LessonType) -> &'static str {
    match lesson_type {
        LessonType::Live => "live_lesson",
        LessonType::Async => "async_lesson",
    }
}

/// `event` の属性（JSON のオブジェクト）。時刻が RFC 3339 で書けない（0〜9999 年の外）ときは失敗する。
pub fn attributes(event: &AnalyticsEvent) -> Result<Value, time::error::Format> {
    Ok(match event {
        AnalyticsEvent::AccountRegistered {
            student_id,
            sport,
            occurred_at,
        } => json!({
            "student_id": account_id_uuid::format(*student_id),
            "sport": sport_value(*sport),
            "occurred_at": date_time(*occurred_at)?,
        }),
        AnalyticsEvent::SlotPublished {
            coach_id,
            slot_id,
            starts_at,
            occurred_at,
        } => json!({
            "coach_id": account_id_uuid::format(*coach_id),
            "slot_id": slot_id_uuid::format(*slot_id),
            "starts_at": date_time(*starts_at)?,
            "occurred_at": date_time(*occurred_at)?,
        }),
        AnalyticsEvent::BookingCreated {
            student_id,
            coach_id,
            booking_id,
            slot_id,
            is_trial,
            occurred_at,
        } => json!({
            "student_id": account_id_uuid::format(*student_id),
            "coach_id": account_id_uuid::format(*coach_id),
            "booking_id": booking_id_uuid::format(*booking_id),
            "slot_id": slot_id_uuid::format(*slot_id),
            "is_trial": is_trial,
            "occurred_at": date_time(*occurred_at)?,
        }),
        AnalyticsEvent::BookingConfirmed {
            student_id,
            coach_id,
            booking_id,
            slot_id,
            occurred_at,
        } => json!({
            "student_id": account_id_uuid::format(*student_id),
            "coach_id": account_id_uuid::format(*coach_id),
            "booking_id": booking_id_uuid::format(*booking_id),
            "slot_id": slot_id_uuid::format(*slot_id),
            "occurred_at": date_time(*occurred_at)?,
        }),
        AnalyticsEvent::BookingCancelled {
            student_id,
            coach_id,
            booking_id,
            slot_id,
            hours_before_start,
            occurred_at,
        } => json!({
            "student_id": account_id_uuid::format(*student_id),
            "coach_id": account_id_uuid::format(*coach_id),
            "booking_id": booking_id_uuid::format(*booking_id),
            "slot_id": slot_id_uuid::format(*slot_id),
            "hours_before_start": hours_before_start,
            "occurred_at": date_time(*occurred_at)?,
        }),
        AnalyticsEvent::CallStarted {
            student_id,
            coach_id,
            booking_id,
            session_id,
            occurred_at,
        } => json!({
            "student_id": account_id_uuid::format(*student_id),
            "coach_id": account_id_uuid::format(*coach_id),
            "booking_id": booking_id_uuid::format(*booking_id),
            "session_id": session_id_uuid::format(*session_id),
            "occurred_at": date_time(*occurred_at)?,
        }),
        AnalyticsEvent::CallEnded {
            student_id,
            coach_id,
            booking_id,
            session_id,
            duration_sec,
            packet_loss_rate,
            jitter_ms,
            round_trip_ms,
            disconnect_count,
            occurred_at,
        } => json!({
            "student_id": account_id_uuid::format(*student_id),
            "coach_id": account_id_uuid::format(*coach_id),
            "booking_id": booking_id_uuid::format(*booking_id),
            "session_id": session_id_uuid::format(*session_id),
            "duration_sec": duration_sec,
            "packet_loss_rate": packet_loss_rate,
            "jitter_ms": jitter_ms,
            "round_trip_ms": round_trip_ms,
            "disconnect_count": disconnect_count,
            "occurred_at": date_time(*occurred_at)?,
        }),
        AnalyticsEvent::VideoSubmitted {
            student_id,
            coach_id,
            video_id,
            sport,
            occurred_at,
        } => json!({
            "student_id": account_id_uuid::format(*student_id),
            "coach_id": account_id_uuid::format(*coach_id),
            "video_id": video_id_uuid::format(*video_id),
            "sport": sport_value(*sport),
            "occurred_at": date_time(*occurred_at)?,
        }),
        AnalyticsEvent::VideoReviewReturned {
            student_id,
            coach_id,
            video_id,
            review_id,
            submitted_at,
            occurred_at,
        } => json!({
            "student_id": account_id_uuid::format(*student_id),
            "coach_id": account_id_uuid::format(*coach_id),
            "video_id": video_id_uuid::format(*video_id),
            "review_id": video_review_id_uuid::format(*review_id),
            "submitted_at": date_time(*submitted_at)?,
            "occurred_at": date_time(*occurred_at)?,
        }),
        AnalyticsEvent::LessonCompleted {
            student_id,
            coach_id,
            lesson_id,
            lesson_type,
            is_first_lesson,
            occurred_at,
        } => json!({
            "student_id": account_id_uuid::format(*student_id),
            "coach_id": account_id_uuid::format(*coach_id),
            "lesson_id": lesson_id_uuid::format(*lesson_id),
            "lesson_type": lesson_type_value(*lesson_type),
            "is_first_lesson": is_first_lesson,
            "occurred_at": date_time(*occurred_at)?,
        }),
        AnalyticsEvent::SubscriptionStarted {
            student_id,
            subscription_id,
            sport,
            occurred_at,
        } => json!({
            "student_id": account_id_uuid::format(*student_id),
            "subscription_id": subscription_id_uuid::format(*subscription_id),
            "sport": sport_value(*sport),
            "occurred_at": date_time(*occurred_at)?,
        }),
        AnalyticsEvent::ReviewSubmitted {
            student_id,
            coach_id,
            lesson_id,
            review_id,
            rating,
            occurred_at,
        } => json!({
            "student_id": account_id_uuid::format(*student_id),
            "coach_id": account_id_uuid::format(*coach_id),
            "lesson_id": lesson_id_uuid::format(*lesson_id),
            "review_id": lesson_review_id_uuid::format(*review_id),
            "rating": rating.value(),
            "occurred_at": date_time(*occurred_at)?,
        }),
    })
}

/// 行動トラッキングに送る 1 行（JSON Lines の 1 行。末尾の改行は含まない）。
/// `{"name": <events.yaml の name>, "attributes": <属性>}`。属性は [`attributes`] に、`common` のうち値が
/// 分かるもの（events.yaml の `common_optional`。`release_id`・`app_version`）を足したもの。
pub fn line(
    event: &AnalyticsEvent,
    common: &AnalyticsCommonAttributes,
) -> Result<String, time::error::Format> {
    let mut attributes = attributes(event)?;
    if let Some(release_id) = &common.release_id {
        attributes["release_id"] = json!(release_id);
    }
    if let Some(app_version) = &common.app_version {
        attributes["app_version"] = json!(app_version);
    }
    Ok(json!({ "name": event.name(), "attributes": attributes }).to_string())
}
