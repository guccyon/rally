//! 契約（OpenAPI）の文字列と domain の値の対応表。HTTP の入出力はこの表だけを通す。
//!
//! アカウント ID・スロット ID（`format: uuid`）のテキスト形式は `application::identity::account_id_uuid`
//! と `application::scheduling::slot_id_uuid`。

use std::time::SystemTime;

use application::scheduling::{SlotCursor, slot_id_uuid};
use domain::{
    async_lesson::{VideoContentType, VideoStatus},
    entitlement::{ContractType, SubscriptionPlan, SubscriptionStatus, YearMonth},
    identity::{Role, Sport},
    lesson::LessonType,
    live_lesson::SessionStatus,
    messaging::{MessageAuthor, MessageKind, MessageSender, Stamp},
    scheduling::{BookingStatus, SlotStatus, SlotSupplyStatus},
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
        Sport::Swimming => "swimming",
    }
}

pub(crate) fn sport_from_contract(value: &str) -> Option<Sport> {
    Sport::ALL
        .into_iter()
        .find(|sport| sport_to_contract(*sport) == value)
}

/// student-app の `Sport`（ゴルフを除く 5 種類）。ゴルフは golf-app の範囲なので 422。
pub(crate) fn student_app_sport(value: &str) -> Result<Sport, ApiError> {
    match sport_from_contract(value) {
        Some(Sport::Golf) => Err(ApiError::validation_failed(
            "student-app はゴルフを扱わない（golf-app の範囲）",
        )),
        Some(sport) => Ok(sport),
        None => Err(unknown_sport(value)),
    }
}

/// admin-web の `Sport`（6 種類すべて）。
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

/// admin-web の `SlotSupplyStatus` の値。
pub(crate) const fn slot_supply_status_to_contract(status: SlotSupplyStatus) -> &'static str {
    match status {
        SlotSupplyStatus::Sufficient => "sufficient",
        SlotSupplyStatus::Insufficient => "insufficient",
    }
}

/// admin-web の `AccountRole` の値（`Account.role`）。
pub(crate) const fn role_to_contract(role: Role) -> &'static str {
    match role {
        Role::Student => "student",
        Role::Coach => "coach",
        Role::Headquarters => "headquarters",
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

/// student-app の `LiveLessonSessionStatus` の値。
pub(crate) const fn session_status_to_contract(status: SessionStatus) -> &'static str {
    match status {
        SessionStatus::InProgress => "in_progress",
        SessionStatus::Ended => "ended",
    }
}

/// student-app の `LessonType` の値（`RecordLessonCompletionRequest` の判別子と `Lesson.lesson_type`）。
pub(crate) const fn lesson_type_to_contract(lesson_type: LessonType) -> &'static str {
    match lesson_type {
        LessonType::Live => "live_lesson",
        LessonType::Async => "async_lesson",
    }
}

/// `lesson_type` を読む。契約の enum にない値は 422。
pub(crate) fn lesson_type_from_contract(value: &str) -> Result<LessonType, ApiError> {
    LessonType::ALL
        .into_iter()
        .find(|lesson_type| lesson_type_to_contract(*lesson_type) == value)
        .ok_or_else(|| ApiError::validation_failed(format!("未知のレッスンの種類: {value}")))
}

/// student-app の `SubmitVideoRequest.content_type` の値。
pub(crate) const fn video_content_type_to_contract(content_type: VideoContentType) -> &'static str {
    match content_type {
        VideoContentType::Mp4 => "video/mp4",
        VideoContentType::QuickTime => "video/quicktime",
    }
}

/// `content_type` を読む。契約の enum にない形式は 422。
pub(crate) fn video_content_type_from_contract(value: &str) -> Result<VideoContentType, ApiError> {
    VideoContentType::ALL
        .into_iter()
        .find(|content_type| video_content_type_to_contract(*content_type) == value)
        .ok_or_else(|| ApiError::validation_failed(format!("未知の動画の形式: {value}")))
}

/// 送付のレスポンスの student-app の `Video.status`（`VideoStatus`）。動画本体のアップロードの完了は Gen1 では
/// 受け取らない（保存先はモック）ので、送付した直後の `awaiting_upload` だけを返す。
pub(crate) const VIDEO_STATUS_AWAITING_UPLOAD: &str = "awaiting_upload";

/// student-app・admin-web の `VideoReviewStatus` の値。添削を返すまでは `in_progress`（未返却）。
pub(crate) const fn video_review_status_to_contract(status: VideoStatus) -> &'static str {
    match status {
        VideoStatus::Submitted => "in_progress",
        VideoStatus::Reviewed => "returned",
    }
}

/// student-app・admin-web の `MessageSenderRole` の値（`Message.sender_role`）。
pub(crate) const fn message_author_to_contract(author: MessageAuthor) -> &'static str {
    match author {
        MessageAuthor::Participant(MessageSender::Student) => "student",
        MessageAuthor::Participant(MessageSender::Coach) => "coach",
        MessageAuthor::System => "system",
    }
}

/// 送り手がシステム（`sender_role` が `system`）のメッセージの `Message.sender_id`。どのアカウントでもない
/// nil UUID（契約の `Message.sender_id` の説明）。
pub(crate) const SYSTEM_MESSAGE_SENDER_ID: &str = "00000000-0000-0000-0000-000000000000";

/// student-app・admin-web の `MessageKind` の値（`Message.kind`）。
pub(crate) const fn message_kind_to_contract(kind: MessageKind) -> &'static str {
    match kind {
        MessageKind::Text => "text",
        MessageKind::VideoReviewRequest => "video_review_request",
        MessageKind::VideoReviewReturn => "video_review_returned",
        MessageKind::Stamp => "stamp",
    }
}

/// student-app・admin-web の `StampId` の値（`Message.stamp_id`・`SendMessageRequest.stamp_id`）。
pub(crate) const fn stamp_to_contract(stamp: Stamp) -> &'static str {
    match stamp {
        Stamp::Thanks => "thanks",
        Stamp::Good => "good",
        Stamp::GotIt => "got_it",
        Stamp::LetsGo => "lets_go",
        Stamp::NiceShot => "nice_shot",
        Stamp::Tired => "tired",
        Stamp::Question => "question",
        Stamp::Ok => "ok",
    }
}

/// `stamp_id` を読む。契約の enum にない ID は 422。
pub(crate) fn stamp_from_contract(value: &str) -> Result<Stamp, ApiError> {
    Stamp::ALL
        .into_iter()
        .find(|stamp| stamp_to_contract(*stamp) == value)
        .ok_or_else(|| ApiError::validation_failed(format!("未知のスタンプ: {value}")))
}

/// student-app の `SubscriptionStatus` の値。
pub(crate) const fn subscription_status_to_contract(status: SubscriptionStatus) -> &'static str {
    match status {
        SubscriptionStatus::Active => "active",
        SubscriptionStatus::Cancelled => "cancelled",
    }
}

/// student-app の `SubscriptionPlan` の値。
pub(crate) const fn subscription_plan_to_contract(plan: SubscriptionPlan) -> &'static str {
    match plan {
        SubscriptionPlan::Standard => "standard",
        SubscriptionPlan::Plus => "plus",
    }
}

/// `plan` を読む。省いたら既定のプラン（standard）。契約の enum にない値は 422。
pub(crate) fn subscription_plan_from_contract(
    value: Option<&str>,
) -> Result<SubscriptionPlan, ApiError> {
    let Some(value) = value else {
        return Ok(SubscriptionPlan::default());
    };
    SubscriptionPlan::ALL
        .into_iter()
        .find(|plan| subscription_plan_to_contract(*plan) == value)
        .ok_or_else(|| ApiError::validation_failed(format!("未知の契約プラン: {value}")))
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
    fn session_statuses_are_in_progress_and_ended() {
        assert_eq!(
            session_status_to_contract(SessionStatus::InProgress),
            "in_progress"
        );
        assert_eq!(session_status_to_contract(SessionStatus::Ended), "ended");
    }

    #[test]
    fn lesson_types_are_live_lesson_and_async_lesson() {
        assert_eq!(lesson_type_to_contract(LessonType::Live), "live_lesson");
        assert_eq!(lesson_type_to_contract(LessonType::Async), "async_lesson");
        for lesson_type in LessonType::ALL {
            assert_eq!(
                lesson_type_from_contract(lesson_type_to_contract(lesson_type)).unwrap(),
                lesson_type
            );
        }
        for value in ["", "live", "LIVE_LESSON"] {
            assert!(lesson_type_from_contract(value).is_err(), "{value:?}");
        }
    }

    #[test]
    fn video_content_types_are_mp4_and_quicktime() {
        for content_type in VideoContentType::ALL {
            assert_eq!(
                video_content_type_from_contract(video_content_type_to_contract(content_type))
                    .unwrap(),
                content_type
            );
        }
        assert_eq!(
            video_content_type_to_contract(VideoContentType::QuickTime),
            "video/quicktime"
        );
        for value in ["", "video/webm", "VIDEO/MP4"] {
            assert!(
                video_content_type_from_contract(value).is_err(),
                "{value:?}"
            );
        }
    }

    #[test]
    fn video_review_statuses_are_in_progress_and_returned() {
        assert_eq!(
            video_review_status_to_contract(VideoStatus::Submitted),
            "in_progress"
        );
        assert_eq!(
            video_review_status_to_contract(VideoStatus::Reviewed),
            "returned"
        );
    }

    #[test]
    fn message_authors_are_student_coach_and_system() {
        assert_eq!(
            MessageAuthor::ALL.map(message_author_to_contract),
            ["student", "coach", "system"]
        );
        assert_eq!(
            application::identity::account_id_uuid::parse(SYSTEM_MESSAGE_SENDER_ID),
            Ok(domain::identity::AccountId::from_u128(0)),
            "システムの送り手は nil UUID"
        );
    }

    #[test]
    fn message_kinds_are_text_the_video_review_request_and_return_and_stamp() {
        assert_eq!(
            MessageKind::ALL.map(message_kind_to_contract),
            [
                "text",
                "video_review_request",
                "video_review_returned",
                "stamp"
            ]
        );
    }

    #[test]
    fn stamps_are_the_eight_ids_and_unknown_ids_are_422() {
        assert_eq!(
            Stamp::ALL.map(stamp_to_contract),
            [
                "thanks",
                "good",
                "got_it",
                "lets_go",
                "nice_shot",
                "tired",
                "question",
                "ok"
            ]
        );
        for stamp in Stamp::ALL {
            assert_eq!(
                stamp_from_contract(stamp_to_contract(stamp)).ok(),
                Some(stamp)
            );
        }
        for value in ["heart", "Thanks", ""] {
            assert!(stamp_from_contract(value).is_err(), "{value:?}");
        }
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
    fn subscription_plans_are_standard_and_plus_and_default_to_standard() {
        for plan in SubscriptionPlan::ALL {
            assert_eq!(
                subscription_plan_from_contract(Some(subscription_plan_to_contract(plan))).ok(),
                Some(plan)
            );
        }
        assert_eq!(
            subscription_plan_to_contract(SubscriptionPlan::Standard),
            "standard"
        );
        assert_eq!(
            subscription_plan_to_contract(SubscriptionPlan::Plus),
            "plus"
        );
        assert_eq!(
            subscription_plan_from_contract(None).ok(),
            Some(SubscriptionPlan::Standard)
        );
        for value in ["", "Plus", "premium"] {
            assert!(
                subscription_plan_from_contract(Some(value)).is_err(),
                "{value:?}"
            );
        }
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
