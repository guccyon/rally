use std::future::Future;

use domain::scheduling::{Booking, BookingId};

use crate::Unavailable;

/// live-lesson が scheduling に予約を問い合わせるポート（通話は確定した予約にしか開始できない）。
///
/// 予約の持ち主は scheduling で、live-lesson は読むだけ。確定済みかの判定は domain の
/// [`domain::live_lesson::LiveLessonSession::start`] が行う。実装は
/// [`crate::scheduling::SchedulingBookingLookup`]。
pub trait BookingLookup: Send + Sync {
    /// `booking_id` の予約。なければ `None`。
    fn find_booking(
        &self,
        booking_id: BookingId,
    ) -> impl Future<Output = Result<Option<Booking>, Unavailable>> + Send;
}
