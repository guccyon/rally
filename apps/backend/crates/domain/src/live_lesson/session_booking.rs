use crate::{identity::AccountId, scheduling::BookingId};

/// 通話セッションに結び付いた確定した予約と、その予約の生徒。
///
/// マンツーマン（定員 1）の枠のセッションには 1 つ、グループレッスン（定員 2 以上。変更シナリオ cs-004）の枠の
/// セッションには、通話を始めたときに確定していた同じ枠の予約の数だけ結び付く
/// （[`super::LiveLessonSession::start`]）。完了したレッスンは結び付いた予約ごとに発行する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionBooking {
    booking_id: BookingId,
    student_id: AccountId,
}

impl SessionBooking {
    pub const fn new(booking_id: BookingId, student_id: AccountId) -> Self {
        Self {
            booking_id,
            student_id,
        }
    }

    pub const fn booking_id(&self) -> BookingId {
        self.booking_id
    }

    pub const fn student_id(&self) -> AccountId {
        self.student_id
    }
}
