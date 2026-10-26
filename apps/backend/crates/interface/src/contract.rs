//! 契約（OpenAPI）の文字列と domain の値の対応表。HTTP の入出力はこの表だけを通す。
//!
//! アカウント ID・スロット ID（`format: uuid`）のテキスト形式は `application::identity::account_id_uuid`
//! と `application::scheduling::slot_id_uuid`。

use std::time::SystemTime;

use application::scheduling::{SlotCursor, slot_id_uuid};
use domain::{
    entitlement::{ContractType, SubscriptionStatus, YearMonth},
    identity::Sport,
    scheduling::{BookingStatus, SlotStatus},
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::error::ApiError;

/// OpenAPI の `Sport` の値。どのクライアントがどれを受け付けるかは各ハンドラーが決める
/// （student-app はゴルフを含まない）。
pub(crate) const fn sport_to_contract(sport: Sport) -> &'static str {
    match sport {
        Sport::Tennis => "tennis",
        Sport::Golf => "golf",
        Sport::Baseball => "baseball",
        Sport::Soccer => "soccer",
        Sport::Basketball => "basketball",
    }
}

pub(crate) fn sport_from_contract(value: &str) -> Option<Sport> {
    Sport::ALL
        .into_iter()
        .find(|sport| sport_to_contract(*sport) == value)
}

/// student-app の `Sport`（ゴルフを除く 4 種類）。ゴルフは golf-app の範囲なので 422。
pub(crate) fn student_app_sport(value: &str) -> Result<Sport, ApiError> {
    match sport_from_contract(value) {
        Some(Sport::Golf) => Err(ApiError::validation_failed(
            "student-app はゴルフを扱わない（golf-app の範囲）",
        )),
        Some(sport) => Ok(sport),
        None => Err(unknown_sport(value)),
    }
}

/// admin-web の `Sport`（5 種類すべて）。
pub(crate) fn admin_web_sport(value: &str) -> Result<Sport, ApiError> {
    sport_from_contract(value).ok_or_else(|| unknown_sport(value))
}

fn unknown_sport(value: &str) -> ApiError {
    ApiError::validation_failed(format!("未知のスポーツ: {value}"))
}

/// admin-web の `SlotStatus` の値。
pub(crate) const fn slot_status_to_contract(status: SlotStatus) -> &'static str {
    match status {
        SlotStatus::Open => "open",
        SlotStatus::Booked => "booked",
        SlotStatus::Closed => "closed",
    }
}

/// student-app の `BookingStatus` の値。
pub(crate) const fn booking_status_to_contract(status: BookingStatus) -> &'static str {
    match status {
        BookingStatus::Pending => "pending",
        BookingStatus::Confirmed => "confirmed",
        BookingStatus::Cancelled => "cancelled",
    }
}

/// student-app の `SubscriptionStatus` の値。
pub(crate) const fn subscription_status_to_contract(status: SubscriptionStatus) -> &'static str {
    match status {
        SubscriptionStatus::Active => "active",
        SubscriptionStatus::Cancelled => "cancelled",
    }
}

/// student-app の `ContractType` の値。月額は `subscription`。
pub(crate) const fn contract_type_to_contract(contract_type: ContractType) -> &'static str {
    match contract_type {
        ContractType::Monthly => "subscription",
        ContractType::OneShot => "one_shot",
    }
}

/// `LiveLessonQuota.month` の値（`YYYY-MM`）。
pub(crate) fn year_month_to_contract(month: YearMonth) -> String {
    format!("{:04}-{:02}", month.year(), month.month())
}

/// `format: date-time`（RFC 3339）で書く。
pub(crate) fn date_time_to_contract(at: SystemTime) -> Result<String, ApiError> {
    OffsetDateTime::from(at)
        .format(&Rfc3339)
        .map_err(ApiError::internal)
}

/// `format: date-time`（RFC 3339）を読む。`field` は 422 の文言に使う項目名。
pub(crate) fn date_time_from_contract(field: &str, value: &str) -> Result<SystemTime, ApiError> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map(SystemTime::from)
        .map_err(|_| ApiError::validation_failed(format!("{field} は RFC 3339 の日時でない")))
}

/// `SlotPage.next_cursor` の値（クライアントには不透明）。`<開始時刻の UNIX ナノ秒>_<スロット ID>`。
pub(crate) fn slot_cursor_to_contract(cursor: SlotCursor) -> String {
    format!(
        "{}_{}",
        OffsetDateTime::from(cursor.starts_at).unix_timestamp_nanos(),
        slot_id_uuid::format(cursor.slot_id)
    )
}

/// `cursor` のクエリを読む。形が違えば 422。
pub(crate) fn slot_cursor_from_contract(value: &str) -> Result<SlotCursor, ApiError> {
    let invalid = || ApiError::validation_failed("cursor が不正");
    let (nanos, slot_id) = value.split_once('_').ok_or_else(invalid)?;
    let nanos: i128 = nanos.parse().map_err(|_| invalid())?;
    let starts_at = OffsetDateTime::from_unix_timestamp_nanos(nanos).map_err(|_| invalid())?;
    Ok(SlotCursor {
        starts_at: starts_at.into(),
        slot_id: slot_id_uuid::parse(slot_id).map_err(|_| invalid())?,
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use domain::scheduling::SlotId;

    use super::*;

    #[test]
    fn parses_the_five_blueprint_sports() {
        for sport in Sport::ALL {
            assert_eq!(sport_from_contract(sport_to_contract(sport)), Some(sport));
        }
    }

    #[test]
    fn unknown_sport_is_none() {
        for value in ["cricket", "Tennis", "", " tennis"] {
            assert_eq!(sport_from_contract(value), None, "{value:?}");
        }
    }

    #[test]
    fn student_app_does_not_accept_golf_but_admin_web_does() {
        assert!(student_app_sport("golf").is_err());
        assert_eq!(student_app_sport("tennis").unwrap(), Sport::Tennis);
        assert_eq!(admin_web_sport("golf").unwrap(), Sport::Golf);
        assert!(admin_web_sport("cricket").is_err());
    }

    #[test]
    fn contract_types_are_subscription_and_one_shot() {
        assert_eq!(
            contract_type_to_contract(ContractType::Monthly),
            "subscription"
        );
        assert_eq!(contract_type_to_contract(ContractType::OneShot), "one_shot");
    }

    #[test]
    fn subscription_statuses_are_active_and_cancelled() {
        assert_eq!(
            subscription_status_to_contract(SubscriptionStatus::Active),
            "active"
        );
        assert_eq!(
            subscription_status_to_contract(SubscriptionStatus::Cancelled),
            "cancelled"
        );
    }

    #[test]
    fn year_month_is_yyyy_mm() {
        assert_eq!(
            year_month_to_contract(YearMonth::new(2026, 3).unwrap()),
            "2026-03"
        );
        assert_eq!(
            year_month_to_contract(YearMonth::new(2026, 12).unwrap()),
            "2026-12"
        );
    }

    #[test]
    fn date_time_round_trips_with_offset() {
        let at = date_time_from_contract("starts_at", "2026-11-01T19:00:00+09:00").unwrap();
        assert_eq!(date_time_to_contract(at).unwrap(), "2026-11-01T10:00:00Z");
        assert!(date_time_from_contract("starts_at", "2026-11-01 10:00").is_err());
    }

    #[test]
    fn slot_cursor_round_trips() {
        let cursor = SlotCursor {
            starts_at: SystemTime::UNIX_EPOCH + Duration::new(1_793_491_200, 123_456_789),
            slot_id: SlotId::from_u128(0x0123_4567_89ab_4def_8123_4567_89ab_cdef),
        };
        let text = slot_cursor_to_contract(cursor);
        assert_eq!(
            text,
            "1793491200123456789_01234567-89ab-4def-8123-456789abcdef"
        );
        assert_eq!(slot_cursor_from_contract(&text).unwrap(), cursor);
        for value in [
            "",
            "abc",
            "1_not-a-uuid",
            "x_01234567-89ab-4def-8123-456789abcdef",
        ] {
            assert!(slot_cursor_from_contract(value).is_err(), "{value:?}");
        }
    }
}
