use std::{future::Future, time::SystemTime};

use domain::{
    DomainError,
    identity::AccountId,
    scheduling::{Booking, BookingId, Slot, SlotId},
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

    /// `slot_id` のスロットと、既存の予約（少なくともそのスロットの予約と `student_id` の生徒の予約）を
    /// 読み、`reserve` に渡す。`reserve` が `Ok` なら返した予約を保存して返す。
    fn reserve<F>(
        &self,
        slot_id: SlotId,
        student_id: AccountId,
        reserve: F,
    ) -> impl Future<Output = Result<Booking, ReserveSlotError>> + Send
    where
        F: FnOnce(&Slot, &[Booking]) -> Result<Booking, DomainError> + Send;

    /// `booking_id` の予約と、そのスロットを読み、`update` に渡す。`update` が `Ok` なら予約と
    /// スロットの両方を保存し、保存した予約を返す。
    fn update<F>(
        &self,
        booking_id: BookingId,
        update: F,
    ) -> impl Future<Output = Result<Booking, UpdateBookingError>> + Send
    where
        F: FnOnce(&mut Booking, &mut Slot) -> Result<(), DomainError> + Send;

    /// 開始が `from`（含む）から `until`（含まない）までの枠（状態を問わない）と、それらの枠への予約
    /// （状態を問わない）。読むだけで何も変えない。予約枠の需給（[`super::GetSlotSupply`]）が数える。
    fn slots_starting_between(
        &self,
        from: SystemTime,
        until: SystemTime,
    ) -> impl Future<Output = Result<(Vec<Slot>, Vec<Booking>), Unavailable>> + Send;
}
