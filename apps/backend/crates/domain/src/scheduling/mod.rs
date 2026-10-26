//! scheduling: 予約（specs/context-map.yaml）。
//!
//! コーチが公開する予約枠（glossary の slot。[`Slot`]）と、同じコーチの枠が重ならないことを
//! 判定する集約（[`CoachCalendar`]）。生徒によるスロットの確保（glossary の booking。[`Booking`]）。
//! 体験予約も予約の 1 つ（`is_trial`）。キャンセルで回数を消費するかの判定（キャンセル規定）は
//! [`CancellationPolicy`] の 1 か所だけに置く（docs/adr/0002 Q2）。

mod booking;
mod booking_id;
mod booking_status;
mod cancellation_decision;
mod cancellation_policy;
mod coach_calendar;
mod slot;
mod slot_id;
mod slot_status;

pub use booking::Booking;
pub use booking_id::BookingId;
pub use booking_status::BookingStatus;
pub use cancellation_decision::CancellationDecision;
pub use cancellation_policy::CancellationPolicy;
pub use coach_calendar::CoachCalendar;
pub use slot::Slot;
pub use slot_id::SlotId;
pub use slot_status::SlotStatus;
