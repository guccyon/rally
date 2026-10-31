use std::{future::Future, time::SystemTime};

use domain::{
    identity::AccountId,
    scheduling::{Booking, BookingId, SchedulingError, Slot, SlotId},
};

use super::{ReserveSlotError, UpdateBookingError};
use crate::Unavailable;

/// 予約の保存先のポート。予約とスロットの状態を一緒に読み書きする（予約の確定でスロットが埋まる）。
/// 実装はテスト・ローカル起動用の [`crate::in_memory::InMemoryBookingRepository`]（Postgres の
/// `scheduling.bookings` の実装は、アカウント・スロットの保存先を Postgres にするときに足す）。
///
/// 不変条件の判定はクロージャ（domain の [`Booking`]）だけが行い、保存先は判定しない。
/// その代わり、読み取りから保存までを直列にする（並行な予約が互いを見ずに両方保存されることがないように）。
pub trait BookingRepository: Send + Sync {
    /// 新しい予約の ID を払い出す。
    fn next_id(&self) -> BookingId;

    /// `booking_id` の予約。なければ `None`。読むだけで何も変えない。
    fn find(
        &self,
        booking_id: BookingId,
    ) -> impl Future<Output = Result<Option<Booking>, Unavailable>> + Send;

    /// `slot_id` のスロット（状態を問わない）。なければ `None`。読むだけで何も変えない。予約の作成
    /// （[`super::CreateBooking`]）が、スロットに結びついたレッスンメニューを catalog に引くために先に読む。
    fn find_slot(
        &self,
        slot_id: SlotId,
    ) -> impl Future<Output = Result<Option<Slot>, Unavailable>> + Send;

    /// `slot_id` のスロットと、既存の予約（少なくともそのスロットの予約と `student_id` の生徒の予約）を
    /// 読み、`reserve` に渡す。`reserve` が `Ok` なら返した予約を保存して返す。スロットの席の数
    /// （[`domain::scheduling::SlotSeats`]）は予約から数えるので、`reserve` が押さえた席は保存しない。
    fn reserve<F>(
        &self,
        slot_id: SlotId,
        student_id: AccountId,
        reserve: F,
    ) -> impl Future<Output = Result<Booking, ReserveSlotError>> + Send
    where
        F: FnOnce(&mut Slot, &[Booking]) -> Result<Booking, SchedulingError> + Send;

    /// `booking_id` の予約と、そのスロットを読み、`update` に渡す。`update` が `Ok` なら予約と
    /// スロットの両方を保存し、保存した予約を返す。
    fn update<F>(
        &self,
        booking_id: BookingId,
        update: F,
    ) -> impl Future<Output = Result<Booking, UpdateBookingError>> + Send
    where
        F: FnOnce(&mut Booking, &mut Slot) -> Result<(), SchedulingError> + Send;

    /// 開始が `from`（含む）から `until`（含まない）までの枠（状態を問わない）と、それらの枠への予約
    /// （状態を問わない）。読むだけで何も変えない。予約枠の需給（[`super::GetSlotSupply`]）が数える。
    fn slots_starting_between(
        &self,
        from: SystemTime,
        until: SystemTime,
    ) -> impl Future<Output = Result<(Vec<Slot>, Vec<Booking>), Unavailable>> + Send;

    /// `student_id` の生徒の予約（状態を問わない）と、それぞれの予約したスロット。並びは決めない。
    /// 読むだけで何も変えない。本人の予約の一覧（[`super::ListMyBookings`]）が読む。
    fn bookings_of_student(
        &self,
        student_id: AccountId,
    ) -> impl Future<Output = Result<Vec<(Booking, Slot)>, Unavailable>> + Send;

    /// `coach_id` のコーチのスロットへの予約（状態を問わない）と、それぞれの予約したスロット。並びは決めない。
    /// 読むだけで何も変えない。コーチの自分の予約の一覧（[`super::ListCoachBookings`]）が読む。
    fn bookings_of_coach(
        &self,
        coach_id: AccountId,
    ) -> impl Future<Output = Result<Vec<(Booking, Slot)>, Unavailable>> + Send;
}
