//! scheduling: 予約（specs/context-map.yaml）。
//!
//! コーチが公開する予約枠（glossary の slot。[`Slot`]。定員 [`SlotCapacity`] と有効な予約の数 [`SlotSeats`] を持つ）と、同じコーチの枠が重ならないことを
//! 判定する集約（[`CoachCalendar`]）。生徒によるスロットの確保（glossary の booking。[`Booking`]）。
//! 体験予約も予約の 1 つ（`is_trial`）。キャンセルで回数を消費するかの判定（キャンセル規定）は
//! [`CancellationPolicy`] の 1 か所だけに置く（docs/adr/0002 Q2）。スポーツごとの予約枠の需給
//! （枠が足りているか）の数え方と判定は [`SlotSupply`] の 1 か所だけに置く。

mod booking;
mod booking_id;
mod booking_status;
mod cancellation_decision;
mod cancellation_policy;
mod coach_calendar;
mod slot;
mod slot_capacity;
mod slot_id;
mod slot_seats;
mod slot_status;
mod slot_supply;
mod slot_supply_status;

pub use booking::Booking;
pub use booking_id::BookingId;
pub use booking_status::BookingStatus;
pub use cancellation_decision::CancellationDecision;
pub use cancellation_policy::CancellationPolicy;
pub use coach_calendar::CoachCalendar;
pub use slot::Slot;
pub use slot_capacity::SlotCapacity;
pub use slot_id::SlotId;
pub use slot_seats::SlotSeats;
pub use slot_status::SlotStatus;
pub use slot_supply::SlotSupply;
pub use slot_supply_status::SlotSupplyStatus;
