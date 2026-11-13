//! scheduling の値と、DB に保存する文字列の対応表。
//!
//! `scheduling.slots.status`・`scheduling.bookings.status`・`scheduling.bookings.cancelled_by`・
//! `scheduling.bookings.cancel_reason`（CHECK なし）・
//! `scheduling.coach_acceptances.status`（CHECK 制約の値）はこの表だけを通す。キャンセルした人の値は境界間イベント
//! `BookingCancelled` の `cancelled_by`（[`crate::domain_event_payload`]）も同じ。`sport` は
//! [`crate::identity_values::sport_value`] と同じ値。CHECK との一致は `tests/schema.rs` が確かめる。

use domain::scheduling::{
    BookingCancelReason, BookingCanceller, BookingStatus, CoachAcceptanceStatus, SlotStatus,
};

/// `scheduling.slots.status` の値。
pub const fn slot_status_value(status: SlotStatus) -> &'static str {
    match status {
        SlotStatus::Open => "open",
        SlotStatus::Booked => "booked",
        SlotStatus::Closed => "closed",
    }
}

pub fn slot_status_from_value(value: &str) -> Option<SlotStatus> {
    SlotStatus::ALL
        .into_iter()
        .find(|status| slot_status_value(*status) == value)
}

/// `scheduling.bookings.status` の値。
pub const fn booking_status_value(status: BookingStatus) -> &'static str {
    match status {
        BookingStatus::Pending => "pending",
        BookingStatus::Confirmed => "confirmed",
        BookingStatus::Cancelled => "cancelled",
    }
}

pub fn booking_status_from_value(value: &str) -> Option<BookingStatus> {
    BookingStatus::ALL
        .into_iter()
        .find(|status| booking_status_value(*status) == value)
}

/// `scheduling.bookings.cancelled_by` の値（境界間イベント `BookingCancelled` の `cancelled_by` も同じ）。
pub const fn booking_canceller_value(canceller: BookingCanceller) -> &'static str {
    match canceller {
        BookingCanceller::Student => "student",
        BookingCanceller::Coach => "coach",
        BookingCanceller::Headquarters => "headquarters",
    }
}

pub fn booking_canceller_from_value(value: &str) -> Option<BookingCanceller> {
    [
        BookingCanceller::Student,
        BookingCanceller::Coach,
        BookingCanceller::Headquarters,
    ]
    .into_iter()
    .find(|canceller| booking_canceller_value(*canceller) == value)
}

/// `scheduling.bookings.cancel_reason` の値。この列は初版の自由記述の列で値の CHECK を持たない（絞るのは contract）ので、
/// 書くのも読むのもこの表の値だけにする。
pub const fn booking_cancel_reason_value(reason: BookingCancelReason) -> &'static str {
    match reason {
        BookingCancelReason::CoachLeave => "coach_leave",
    }
}

pub fn booking_cancel_reason_from_value(value: &str) -> Option<BookingCancelReason> {
    [BookingCancelReason::CoachLeave]
        .into_iter()
        .find(|reason| booking_cancel_reason_value(*reason) == value)
}

/// `scheduling.coach_acceptances.status` の値。
pub const fn coach_acceptance_status_value(status: CoachAcceptanceStatus) -> &'static str {
    match status {
        CoachAcceptanceStatus::Accepting => "accepting",
        CoachAcceptanceStatus::Suspended => "suspended",
    }
}

pub fn coach_acceptance_status_from_value(value: &str) -> Option<CoachAcceptanceStatus> {
    CoachAcceptanceStatus::ALL
        .into_iter()
        .find(|status| coach_acceptance_status_value(*status) == value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn booking_canceller_values_round_trip() {
        for canceller in [
            BookingCanceller::Student,
            BookingCanceller::Coach,
            BookingCanceller::Headquarters,
        ] {
            assert_eq!(
                booking_canceller_from_value(booking_canceller_value(canceller)),
                Some(canceller)
            );
        }
        assert_eq!(booking_canceller_from_value("system"), None);
    }

    #[test]
    fn booking_cancel_reason_values_round_trip() {
        assert_eq!(
            booking_cancel_reason_from_value(booking_cancel_reason_value(
                BookingCancelReason::CoachLeave
            )),
            Some(BookingCancelReason::CoachLeave)
        );
        assert_eq!(booking_cancel_reason_from_value("payment_failed"), None);
    }

    #[test]
    fn values_round_trip() {
        for status in SlotStatus::ALL {
            assert_eq!(
                slot_status_from_value(slot_status_value(status)),
                Some(status)
            );
        }
        for status in BookingStatus::ALL {
            assert_eq!(
                booking_status_from_value(booking_status_value(status)),
                Some(status)
            );
        }
        for status in CoachAcceptanceStatus::ALL {
            assert_eq!(
                coach_acceptance_status_from_value(coach_acceptance_status_value(status)),
                Some(status)
            );
        }
    }

    #[test]
    fn unknown_values_are_none() {
        for value in ["", "Open", "cancelled", " open"] {
            assert_eq!(slot_status_from_value(value), None, "{value:?}");
        }
        for value in ["", "Pending", "booked", " confirmed"] {
            assert_eq!(booking_status_from_value(value), None, "{value:?}");
        }
        for value in ["", "Suspended", "open", " accepting"] {
            assert_eq!(coach_acceptance_status_from_value(value), None, "{value:?}");
        }
    }
}
