//! scheduling: 予約（specs/context-map.yaml）。
//!
//! コーチが公開する予約枠（glossary の slot。[`Slot`]）と、同じコーチの枠が重ならないことを
//! 判定する集約（[`CoachCalendar`]）。予約（booking）は T08 で足す。

mod coach_calendar;
mod slot;
mod slot_id;
mod slot_status;

pub use coach_calendar::CoachCalendar;
pub use slot::Slot;
pub use slot_id::SlotId;
pub use slot_status::SlotStatus;
