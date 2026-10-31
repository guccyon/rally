//! scheduling（予約と枠）の失敗を、OpenAPI の `Error` の応答に写す。
//!
//! 不変条件の違反（[`SchedulingError`]）の文言と、ユースケースの失敗（予約の作成・確定・キャンセルと枠の公開）の
//! HTTP の状態・`code` はここだけで決める（`/bookings`・`/slots` のハンドラーは変種を個別に拾わない）。同じ変種でも
//! 操作で応答が違うので、ユースケースの失敗ごとに写す。どの写しも変種を漏れなく並べる（変種を足したらここで決める）。
//!
//! - 予約の作成（`createBooking`）: ないスロットと閉じた枠（`SlotNotOpen` の `closed`）は 404 `not_found`、満席・
//!   同じ生徒の 2 回目・公開中でない枠（`SlotAlreadyBooked`・`SlotAlreadyBookedByStudent`・`SlotNotOpen`）は 409
//!   `slot_already_booked`（文言で分ける）、2 回目の体験予約（`TrialAlreadyUsed`）は 409 `trial_already_used`、
//!   それ以外は 422 `validation_failed`
//! - 予約の確定（`confirmBooking`）: ない予約と別の生徒の予約（`BookingOfAnotherStudent`。あることも知らせない）は
//!   404 `not_found`、確定前でない予約・公開中でない枠（`BookingNotPending`・`SlotNotOpen`）は 409
//!   `invalid_booking_state`、ビデオ通話の暦月の枠が残っていない（`LiveLessonQuotaExceeded`）は 409
//!   `live_lesson_quota_exceeded`（受講権の枠の消費と同じ応答。[`entitlement_error::live_lesson_quota_exceeded`]）、
//!   それ以外は確定では起きない（保存した値の破損）ので 500
//! - 予約のキャンセル（`cancelBooking`）: ない予約と別の生徒の予約は 404 `not_found`、キャンセル済み
//!   （`BookingAlreadyCancelled`）は 409 `invalid_booking_state`、それ以外は 500
//! - 枠の公開（`publishSlot`）: 同じコーチの枠と重なる（`SlotOverlaps`）は 409 `slot_overlaps`、結びつけるメニューが
//!   ないのと、それ以外の違反は 422 `validation_failed`
//! - 境界で値を作るとき（定員の検証など。`From<SchedulingError>`）は、どの変種も 422 `validation_failed`
//!
//! 403 に当たる失敗はない（役割は `require_role` の層が確かめ、別の生徒の予約は 404 で隠す）。

use application::scheduling::{
    CancelBookingError, ConfirmBookingError, CreateBookingError, PublishSlotError, slot_id_uuid,
};
use domain::scheduling::{SchedulingError, SlotStatus};

use crate::{
    contract::sport_to_contract,
    entitlement_error,
    error::ApiError,
    message::{self, booking_status_label},
};

/// 予約できるスロットがない 404（閉じた枠も同じ）。
pub(crate) fn slot_not_found() -> ApiError {
    ApiError::not_found("予約できるスロットがない")
}

/// 予約がない 404（別の生徒の予約も同じ）。
pub(crate) fn booking_not_found() -> ApiError {
    ApiError::not_found("予約がない")
}

/// 境界で作った値（定員など）が不変条件を満たさない 422。
impl From<SchedulingError> for ApiError {
    fn from(error: SchedulingError) -> Self {
        Self::validation_failed(describe(&error))
    }
}

impl From<CreateBookingError> for ApiError {
    fn from(error: CreateBookingError) -> Self {
        let error = match error {
            CreateBookingError::SlotNotFound => return slot_not_found(),
            CreateBookingError::Invalid(error) => error,
            CreateBookingError::Unavailable(error) => return Self::internal(error),
        };
        match error {
            // 閉じた枠は生徒の検索に出ないので、ない枠と同じに扱う
            SchedulingError::SlotNotOpen {
                status: SlotStatus::Closed,
            } => slot_not_found(),
            // 満席と、同じ生徒の同じ枠への 2 回目は、どちらも 409 slot_already_booked（文言で分ける）
            SchedulingError::SlotAlreadyBooked
            | SchedulingError::SlotAlreadyBookedByStudent
            | SchedulingError::SlotNotOpen { .. } => {
                Self::conflict("slot_already_booked", describe(&error))
            }
            SchedulingError::TrialAlreadyUsed => {
                Self::conflict("trial_already_used", describe(&error))
            }
            SchedulingError::SlotEndsBeforeItStarts
            | SchedulingError::SlotOverlaps { .. }
            | SchedulingError::SlotOfAnotherCoach
            | SchedulingError::SlotCapacityOutOfRange { .. }
            | SchedulingError::SlotNotBooked { .. }
            | SchedulingError::SlotSeatsOverCapacity { .. }
            | SchedulingError::SlotStatusMismatchesSeats { .. }
            | SchedulingError::BookingNotPending { .. }
            | SchedulingError::BookingAlreadyCancelled
            | SchedulingError::BookingOfAnotherStudent
            | SchedulingError::BookingOfAnotherSlot
            | SchedulingError::LiveLessonQuotaExceeded
            | SchedulingError::LessonMenuOfAnotherCoach
            | SchedulingError::LessonMenuNotPublished
            | SchedulingError::LessonMenuNotLive
            | SchedulingError::LessonMenuSportMismatch { .. }
            | SchedulingError::SlotLengthMismatchesLessonMenu { .. } => Self::from(error),
        }
    }
}

impl From<ConfirmBookingError> for ApiError {
    fn from(error: ConfirmBookingError) -> Self {
        let error = match error {
            ConfirmBookingError::BookingNotFound => return booking_not_found(),
            ConfirmBookingError::Invalid(error) => error,
            ConfirmBookingError::Unavailable(error) => return Self::internal(error),
        };
        match error {
            // 別の生徒の予約は、あることも知らせない
            SchedulingError::BookingOfAnotherStudent => booking_not_found(),
            SchedulingError::BookingNotPending { .. } | SchedulingError::SlotNotOpen { .. } => {
                Self::conflict("invalid_booking_state", describe(&error))
            }
            SchedulingError::LiveLessonQuotaExceeded => {
                entitlement_error::live_lesson_quota_exceeded()
            }
            SchedulingError::SlotEndsBeforeItStarts
            | SchedulingError::SlotOverlaps { .. }
            | SchedulingError::SlotOfAnotherCoach
            | SchedulingError::SlotCapacityOutOfRange { .. }
            | SchedulingError::SlotNotBooked { .. }
            | SchedulingError::SlotAlreadyBooked
            | SchedulingError::SlotSeatsOverCapacity { .. }
            | SchedulingError::SlotStatusMismatchesSeats { .. }
            | SchedulingError::SlotAlreadyBookedByStudent
            | SchedulingError::TrialAlreadyUsed
            | SchedulingError::BookingAlreadyCancelled
            | SchedulingError::BookingOfAnotherSlot
            | SchedulingError::LessonMenuOfAnotherCoach
            | SchedulingError::LessonMenuNotPublished
            | SchedulingError::LessonMenuNotLive
            | SchedulingError::LessonMenuSportMismatch { .. }
            | SchedulingError::SlotLengthMismatchesLessonMenu { .. } => Self::internal(error),
        }
    }
}

impl From<CancelBookingError> for ApiError {
    fn from(error: CancelBookingError) -> Self {
        let error = match error {
            CancelBookingError::BookingNotFound => return booking_not_found(),
            CancelBookingError::Invalid(error) => error,
            CancelBookingError::Unavailable(error) => return Self::internal(error),
        };
        match error {
            // 別の生徒の予約は、あることも知らせない
            SchedulingError::BookingOfAnotherStudent => booking_not_found(),
            SchedulingError::BookingAlreadyCancelled => {
                Self::conflict("invalid_booking_state", describe(&error))
            }
            SchedulingError::SlotEndsBeforeItStarts
            | SchedulingError::SlotOverlaps { .. }
            | SchedulingError::SlotOfAnotherCoach
            | SchedulingError::SlotNotOpen { .. }
            | SchedulingError::SlotCapacityOutOfRange { .. }
            | SchedulingError::SlotNotBooked { .. }
            | SchedulingError::SlotAlreadyBooked
            | SchedulingError::SlotSeatsOverCapacity { .. }
            | SchedulingError::SlotStatusMismatchesSeats { .. }
            | SchedulingError::SlotAlreadyBookedByStudent
            | SchedulingError::TrialAlreadyUsed
            | SchedulingError::BookingNotPending { .. }
            | SchedulingError::BookingOfAnotherSlot
            | SchedulingError::LiveLessonQuotaExceeded
            | SchedulingError::LessonMenuOfAnotherCoach
            | SchedulingError::LessonMenuNotPublished
            | SchedulingError::LessonMenuNotLive
            | SchedulingError::LessonMenuSportMismatch { .. }
            | SchedulingError::SlotLengthMismatchesLessonMenu { .. } => Self::internal(error),
        }
    }
}

impl From<PublishSlotError> for ApiError {
    fn from(error: PublishSlotError) -> Self {
        let error = match error {
            PublishSlotError::Invalid(error) => error,
            PublishSlotError::LessonMenuNotFound => {
                return Self::validation_failed(message::LESSON_MENU_NOT_FOUND);
            }
            PublishSlotError::Unavailable(error) => return Self::internal(error),
        };
        match error {
            SchedulingError::SlotOverlaps { .. } => {
                Self::conflict("slot_overlaps", describe(&error))
            }
            SchedulingError::SlotEndsBeforeItStarts
            | SchedulingError::SlotOfAnotherCoach
            | SchedulingError::SlotNotOpen { .. }
            | SchedulingError::SlotCapacityOutOfRange { .. }
            | SchedulingError::SlotNotBooked { .. }
            | SchedulingError::SlotAlreadyBooked
            | SchedulingError::SlotSeatsOverCapacity { .. }
            | SchedulingError::SlotStatusMismatchesSeats { .. }
            | SchedulingError::SlotAlreadyBookedByStudent
            | SchedulingError::TrialAlreadyUsed
            | SchedulingError::BookingNotPending { .. }
            | SchedulingError::BookingAlreadyCancelled
            | SchedulingError::BookingOfAnotherStudent
            | SchedulingError::BookingOfAnotherSlot
            | SchedulingError::LiveLessonQuotaExceeded
            | SchedulingError::LessonMenuOfAnotherCoach
            | SchedulingError::LessonMenuNotPublished
            | SchedulingError::LessonMenuNotLive
            | SchedulingError::LessonMenuSportMismatch { .. }
            | SchedulingError::SlotLengthMismatchesLessonMenu { .. } => Self::from(error),
        }
    }
}

/// 不変条件の違反の説明（API の `message`）。
fn describe(error: &SchedulingError) -> String {
    match error {
        SchedulingError::SlotEndsBeforeItStarts => "スロットの開始が終了より前でない".to_string(),
        SchedulingError::SlotOverlaps { existing } => format!(
            "同じコーチの枠（{}）と期間が重なる",
            slot_id_uuid::format(*existing)
        ),
        SchedulingError::SlotOfAnotherCoach => "別のコーチの枠は扱えない".to_string(),
        SchedulingError::SlotNotOpen { status } => {
            format!("スロットが公開中でない（{}）", slot_status_label(*status))
        }
        SchedulingError::SlotCapacityOutOfRange { min, max } => {
            format!("スロットの定員は {min}〜{max} の整数")
        }
        SchedulingError::SlotNotBooked { status } => {
            format!("スロットが予約済みでない（{}）", slot_status_label(*status))
        }
        SchedulingError::SlotAlreadyBooked => "スロットはすでに予約されている".to_string(),
        SchedulingError::SlotAlreadyBookedByStudent => {
            "このスロットはすでに予約している".to_string()
        }
        SchedulingError::SlotSeatsOverCapacity { capacity, active } => {
            format!("スロットの有効な予約は定員 {capacity} まで（{active} 件）")
        }
        SchedulingError::SlotStatusMismatchesSeats { status } => format!(
            "スロットの状態と予約の数が合わない（{}）",
            slot_status_label(*status)
        ),
        SchedulingError::TrialAlreadyUsed => "体験予約は初回の予約だけ".to_string(),
        SchedulingError::BookingNotPending { status } => {
            format!("確定待ちの予約でない（{}）", booking_status_label(*status))
        }
        SchedulingError::BookingAlreadyCancelled => "予約はキャンセル済み".to_string(),
        SchedulingError::BookingOfAnotherStudent => "別の生徒の予約は扱えない".to_string(),
        SchedulingError::BookingOfAnotherSlot => "予約と別のスロットは扱えない".to_string(),
        SchedulingError::LiveLessonQuotaExceeded => {
            entitlement_error::LIVE_LESSON_QUOTA_EXCEEDED.to_string()
        }
        SchedulingError::LessonMenuOfAnotherCoach => {
            "枠に結びつけられるのは自分のレッスンメニューだけ".to_string()
        }
        SchedulingError::LessonMenuNotPublished => {
            "公開中でないレッスンメニューは枠に結びつけられない".to_string()
        }
        SchedulingError::LessonMenuNotLive => {
            "非同期レッスンのメニューは枠に結びつけられない（同期レッスンのメニューだけ）"
                .to_string()
        }
        SchedulingError::LessonMenuSportMismatch { menu, slot } => format!(
            "レッスンメニューのスポーツ（{}）が枠のスポーツ（{}）と違う",
            sport_to_contract(*menu),
            sport_to_contract(*slot)
        ),
        SchedulingError::SlotLengthMismatchesLessonMenu { minutes } => {
            format!("枠の長さがレッスンメニューの 1 回の時間（{minutes} 分）と違う")
        }
    }
}

/// スロットの状態の呼び名（`specs/glossary.yaml` の `slot` の `states`。admin-web の画面と同じ）。
const fn slot_status_label(status: SlotStatus) -> &'static str {
    match status {
        SlotStatus::Open => "公開中",
        SlotStatus::Booked => "予約済み",
        SlotStatus::Closed => "終了",
    }
}

#[cfg(test)]
mod tests {
    use application::Unavailable;
    use axum::http::StatusCode;
    use domain::{
        catalog::Sport,
        scheduling::{BookingStatus, SlotId},
    };

    use super::*;

    /// すべての変種と、その文言（[`describe`]）。
    fn every_variant() -> Vec<(SchedulingError, &'static str)> {
        let all = vec![
            (
                SchedulingError::SlotEndsBeforeItStarts,
                "スロットの開始が終了より前でない",
            ),
            (
                SchedulingError::SlotOverlaps {
                    existing: SlotId::from_u128(1),
                },
                "同じコーチの枠（00000000-0000-0000-0000-000000000001）と期間が重なる",
            ),
            (
                SchedulingError::SlotOfAnotherCoach,
                "別のコーチの枠は扱えない",
            ),
            (
                SchedulingError::SlotNotOpen {
                    status: SlotStatus::Booked,
                },
                "スロットが公開中でない（予約済み）",
            ),
            (
                SchedulingError::SlotNotOpen {
                    status: SlotStatus::Closed,
                },
                "スロットが公開中でない（終了）",
            ),
            (
                SchedulingError::SlotCapacityOutOfRange { min: 1, max: 6 },
                "スロットの定員は 1〜6 の整数",
            ),
            (
                SchedulingError::SlotNotBooked {
                    status: SlotStatus::Open,
                },
                "スロットが予約済みでない（公開中）",
            ),
            (
                SchedulingError::SlotAlreadyBooked,
                "スロットはすでに予約されている",
            ),
            (
                SchedulingError::SlotSeatsOverCapacity {
                    capacity: 1,
                    active: 2,
                },
                "スロットの有効な予約は定員 1 まで（2 件）",
            ),
            (
                SchedulingError::SlotStatusMismatchesSeats {
                    status: SlotStatus::Booked,
                },
                "スロットの状態と予約の数が合わない（予約済み）",
            ),
            (
                SchedulingError::SlotAlreadyBookedByStudent,
                "このスロットはすでに予約している",
            ),
            (
                SchedulingError::TrialAlreadyUsed,
                "体験予約は初回の予約だけ",
            ),
            (
                SchedulingError::BookingNotPending {
                    status: BookingStatus::Confirmed,
                },
                "確定待ちの予約でない（確定済み）",
            ),
            (
                SchedulingError::BookingAlreadyCancelled,
                "予約はキャンセル済み",
            ),
            (
                SchedulingError::BookingOfAnotherStudent,
                "別の生徒の予約は扱えない",
            ),
            (
                SchedulingError::BookingOfAnotherSlot,
                "予約と別のスロットは扱えない",
            ),
            (
                SchedulingError::LiveLessonQuotaExceeded,
                "今月のビデオ通話の枠を使い切っている",
            ),
            (
                SchedulingError::LessonMenuOfAnotherCoach,
                "枠に結びつけられるのは自分のレッスンメニューだけ",
            ),
            (
                SchedulingError::LessonMenuNotPublished,
                "公開中でないレッスンメニューは枠に結びつけられない",
            ),
            (
                SchedulingError::LessonMenuNotLive,
                "非同期レッスンのメニューは枠に結びつけられない（同期レッスンのメニューだけ）",
            ),
            (
                SchedulingError::LessonMenuSportMismatch {
                    menu: Sport::Golf,
                    slot: Sport::Tennis,
                },
                "レッスンメニューのスポーツ（golf）が枠のスポーツ（tennis）と違う",
            ),
            (
                SchedulingError::SlotLengthMismatchesLessonMenu { minutes: 60 },
                "枠の長さがレッスンメニューの 1 回の時間（60 分）と違う",
            ),
        ];
        // 変種を足したらここで止まる（上の一覧にも足す）
        for (error, _) in &all {
            match error {
                SchedulingError::SlotEndsBeforeItStarts
                | SchedulingError::SlotOverlaps { .. }
                | SchedulingError::SlotOfAnotherCoach
                | SchedulingError::SlotNotOpen { .. }
                | SchedulingError::SlotCapacityOutOfRange { .. }
                | SchedulingError::SlotNotBooked { .. }
                | SchedulingError::SlotAlreadyBooked
                | SchedulingError::SlotSeatsOverCapacity { .. }
                | SchedulingError::SlotStatusMismatchesSeats { .. }
                | SchedulingError::SlotAlreadyBookedByStudent
                | SchedulingError::TrialAlreadyUsed
                | SchedulingError::BookingNotPending { .. }
                | SchedulingError::BookingAlreadyCancelled
                | SchedulingError::BookingOfAnotherStudent
                | SchedulingError::BookingOfAnotherSlot
                | SchedulingError::LiveLessonQuotaExceeded
                | SchedulingError::LessonMenuOfAnotherCoach
                | SchedulingError::LessonMenuNotPublished
                | SchedulingError::LessonMenuNotLive
                | SchedulingError::LessonMenuSportMismatch { .. }
                | SchedulingError::SlotLengthMismatchesLessonMenu { .. } => {}
            }
        }
        all
    }

    /// `special` に挙げた変種はその状態・`code`・文言、それ以外の変種は `default` に写すことを確かめる。
    /// `default` の文言が `None` なら変種の文言（[`describe`]）。
    fn assert_maps(
        to_api_error: impl Fn(SchedulingError) -> ApiError,
        special: &[(SchedulingError, StatusCode, &str, &str)],
        default: (StatusCode, &str, Option<&str>),
    ) {
        for (error, message) in every_variant() {
            let expected = special
                .iter()
                .find(|(special, ..)| *special == error)
                .map(|(_, status, code, message)| (*status, *code, *message))
                .unwrap_or((default.0, default.1, default.2.unwrap_or(message)));
            let api_error = to_api_error(error.clone());
            assert_eq!(
                (api_error.status(), api_error.code(), api_error.message()),
                expected,
                "{error:?}"
            );
        }
    }

    const INTERNAL: (StatusCode, &str, Option<&str>) = (
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal",
        Some("サーバー内部のエラー"),
    );

    const VALIDATION_FAILED: (StatusCode, &str, Option<&str>) =
        (StatusCode::UNPROCESSABLE_ENTITY, "validation_failed", None);

    #[test]
    fn maps_each_scheduling_error_to_validation_failed() {
        assert_maps(ApiError::from, &[], VALIDATION_FAILED);
    }

    #[test]
    fn maps_each_create_booking_error_to_a_status_and_code() {
        assert_maps(
            |error| ApiError::from(CreateBookingError::Invalid(error)),
            &[
                (
                    SchedulingError::SlotNotOpen {
                        status: SlotStatus::Closed,
                    },
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "予約できるスロットがない",
                ),
                (
                    SchedulingError::SlotNotOpen {
                        status: SlotStatus::Booked,
                    },
                    StatusCode::CONFLICT,
                    "slot_already_booked",
                    "スロットが公開中でない（予約済み）",
                ),
                (
                    SchedulingError::SlotAlreadyBooked,
                    StatusCode::CONFLICT,
                    "slot_already_booked",
                    "スロットはすでに予約されている",
                ),
                (
                    SchedulingError::SlotAlreadyBookedByStudent,
                    StatusCode::CONFLICT,
                    "slot_already_booked",
                    "このスロットはすでに予約している",
                ),
                (
                    SchedulingError::TrialAlreadyUsed,
                    StatusCode::CONFLICT,
                    "trial_already_used",
                    "体験予約は初回の予約だけ",
                ),
            ],
            VALIDATION_FAILED,
        );
        for (error, status, code, message) in [
            (
                CreateBookingError::SlotNotFound,
                StatusCode::NOT_FOUND,
                "not_found",
                "予約できるスロットがない",
            ),
            (
                CreateBookingError::Unavailable(Unavailable::new("db")),
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "サーバー内部のエラー",
            ),
        ] {
            let api_error = ApiError::from(error);
            assert_eq!(
                (api_error.status(), api_error.code(), api_error.message()),
                (status, code, message)
            );
        }
    }

    #[test]
    fn maps_each_confirm_booking_error_to_a_status_and_code() {
        assert_maps(
            |error| ApiError::from(ConfirmBookingError::Invalid(error)),
            &[
                (
                    SchedulingError::BookingOfAnotherStudent,
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "予約がない",
                ),
                (
                    SchedulingError::BookingNotPending {
                        status: BookingStatus::Confirmed,
                    },
                    StatusCode::CONFLICT,
                    "invalid_booking_state",
                    "確定待ちの予約でない（確定済み）",
                ),
                (
                    SchedulingError::SlotNotOpen {
                        status: SlotStatus::Booked,
                    },
                    StatusCode::CONFLICT,
                    "invalid_booking_state",
                    "スロットが公開中でない（予約済み）",
                ),
                (
                    SchedulingError::SlotNotOpen {
                        status: SlotStatus::Closed,
                    },
                    StatusCode::CONFLICT,
                    "invalid_booking_state",
                    "スロットが公開中でない（終了）",
                ),
                (
                    SchedulingError::LiveLessonQuotaExceeded,
                    StatusCode::CONFLICT,
                    "live_lesson_quota_exceeded",
                    "今月のビデオ通話の枠を使い切っている",
                ),
            ],
            INTERNAL,
        );
        for (error, status, code, message) in [
            (
                ConfirmBookingError::BookingNotFound,
                StatusCode::NOT_FOUND,
                "not_found",
                "予約がない",
            ),
            (
                ConfirmBookingError::Unavailable(Unavailable::new("db")),
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "サーバー内部のエラー",
            ),
        ] {
            let api_error = ApiError::from(error);
            assert_eq!(
                (api_error.status(), api_error.code(), api_error.message()),
                (status, code, message)
            );
        }
    }

    #[test]
    fn maps_each_cancel_booking_error_to_a_status_and_code() {
        assert_maps(
            |error| ApiError::from(CancelBookingError::Invalid(error)),
            &[
                (
                    SchedulingError::BookingOfAnotherStudent,
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "予約がない",
                ),
                (
                    SchedulingError::BookingAlreadyCancelled,
                    StatusCode::CONFLICT,
                    "invalid_booking_state",
                    "予約はキャンセル済み",
                ),
            ],
            INTERNAL,
        );
        for (error, status, code, message) in [
            (
                CancelBookingError::BookingNotFound,
                StatusCode::NOT_FOUND,
                "not_found",
                "予約がない",
            ),
            (
                CancelBookingError::Unavailable(Unavailable::new("db")),
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "サーバー内部のエラー",
            ),
        ] {
            let api_error = ApiError::from(error);
            assert_eq!(
                (api_error.status(), api_error.code(), api_error.message()),
                (status, code, message)
            );
        }
    }

    #[test]
    fn maps_each_publish_slot_error_to_a_status_and_code() {
        assert_maps(
            |error| ApiError::from(PublishSlotError::Invalid(error)),
            &[(
                SchedulingError::SlotOverlaps {
                    existing: SlotId::from_u128(1),
                },
                StatusCode::CONFLICT,
                "slot_overlaps",
                "同じコーチの枠（00000000-0000-0000-0000-000000000001）と期間が重なる",
            )],
            VALIDATION_FAILED,
        );
        for (error, status, code, message) in [
            (
                PublishSlotError::LessonMenuNotFound,
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_failed",
                "指定したレッスンメニューがない",
            ),
            (
                PublishSlotError::Unavailable(Unavailable::new("db")),
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "サーバー内部のエラー",
            ),
        ] {
            let api_error = ApiError::from(error);
            assert_eq!(
                (api_error.status(), api_error.code(), api_error.message()),
                (status, code, message)
            );
        }
    }
}
