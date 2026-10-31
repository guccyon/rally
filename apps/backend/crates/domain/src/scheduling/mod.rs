//! scheduling: 予約（specs/context-map.yaml）。
//!
//! コーチが公開する予約枠（glossary の slot。[`Slot`]。定員 [`SlotCapacity`] と有効な予約の数 [`SlotSeats`] を持つ）と、同じコーチの枠が重ならないことを
//! 判定する集約（[`CoachCalendar`]）。枠に結びつけられるレッスンメニュー（catalog）の判定は [`Slot::with_lesson_menu`] の 1 か所。生徒によるスロットの確保（glossary の booking。[`Booking`]）。
//! 体験予約も予約の 1 つ（`is_trial`）。予約は予約した時点の枠のレッスンメニューの写し（[`BookingLessonMenu`]）を持つ。キャンセルで回数を消費するかの判定（キャンセル規定）と、
//! キャンセルする前の見込み（[`CancellationPreview`]）は [`CancellationPolicy`] の 1 か所だけに置く（docs/adr/0002 Q2）。スポーツごとの予約枠の需給
//! （枠が足りているか）の数え方と判定は [`SlotSupply`] の 1 か所だけに置く。本部が見る予約の一覧の期間の規則は [`BookingPeriod`]。
//! コーチが新しい予約を受け付けるか（受付中・停止中。本部が止める・再開する）は [`CoachAcceptance`]、止めた・再開した記録は
//! [`CoachAcceptanceChange`]。不変条件の違反は
//! [`SchedulingError`]。

mod booking;
mod booking_id;
mod booking_lesson_menu;
mod booking_period;
mod booking_status;
mod cancellation_decision;
mod cancellation_policy;
mod cancellation_preview;
mod coach_acceptance;
mod coach_acceptance_change;
mod coach_acceptance_status;
mod coach_calendar;
mod scheduling_error;
mod slot;
mod slot_capacity;
mod slot_id;
mod slot_seats;
mod slot_status;
mod slot_supply;
mod slot_supply_status;

pub use booking::Booking;
pub use booking_id::BookingId;
pub use booking_lesson_menu::BookingLessonMenu;
pub use booking_period::BookingPeriod;
pub use booking_status::BookingStatus;
pub use cancellation_decision::CancellationDecision;
pub use cancellation_policy::CancellationPolicy;
pub use cancellation_preview::CancellationPreview;
pub use coach_acceptance::CoachAcceptance;
pub use coach_acceptance_change::CoachAcceptanceChange;
pub use coach_acceptance_status::CoachAcceptanceStatus;
pub use coach_calendar::CoachCalendar;
pub use scheduling_error::SchedulingError;
pub use slot::Slot;
pub use slot_capacity::SlotCapacity;
pub use slot_id::SlotId;
pub use slot_seats::SlotSeats;
pub use slot_status::SlotStatus;
pub use slot_supply::SlotSupply;
pub use slot_supply_status::SlotSupplyStatus;
