//! ドメインイベントと、境界間イベントのペイロード（specs/contracts/events/asyncapi.yaml）の対応表。
//!
//! ペイロードは AsyncAPI の `required` の属性だけ。ID は UUID のハイフン区切り、時刻（`occurred_at` など）は
//! RFC 3339（`format: date-time`）。契約との一致は `tests/domain_event_payload.rs` が確かめる。

use std::time::SystemTime;

use application::{
    domain_event::DomainEvent, identity::account_id_uuid, scheduling::booking_id_uuid,
};
use serde_json::{Value, json};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

/// `event` のペイロード（JSON）。時刻が RFC 3339 で書けない（0〜9999 年の外）ときは失敗する。
pub fn payload(event: &DomainEvent) -> Result<Value, time::error::Format> {
    match event {
        DomainEvent::BookingConfirmed {
            event_id,
            occurred_at,
            student_id,
            coach_id,
            booking_id,
        } => Ok(json!({
            "event_id": event_id.hyphenated().to_string(),
            "occurred_at": date_time(*occurred_at)?,
            "student_id": account_id_uuid::format(*student_id),
            "coach_id": account_id_uuid::format(*coach_id),
            "booking_id": booking_id_uuid::format(*booking_id),
        })),
        DomainEvent::BookingCancelled {
            event_id,
            occurred_at,
            student_id,
            coach_id,
            booking_id,
            consumes_quota,
            cancelled_at,
        } => Ok(json!({
            "event_id": event_id.hyphenated().to_string(),
            "occurred_at": date_time(*occurred_at)?,
            "student_id": account_id_uuid::format(*student_id),
            "coach_id": account_id_uuid::format(*coach_id),
            "booking_id": booking_id_uuid::format(*booking_id),
            "consumes_quota": consumes_quota,
            "cancelled_at": date_time(*cancelled_at)?,
        })),
    }
}

/// `format: date-time`（RFC 3339）。
fn date_time(at: SystemTime) -> Result<String, time::error::Format> {
    OffsetDateTime::from(at).format(&Rfc3339)
}
