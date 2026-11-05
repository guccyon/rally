use std::{collections::BTreeSet, future::Future};

use domain::{
    identity::AccountId,
    scheduling::{
        Booking, BookingId, BookingPeriod, CoachLeave, CoachLeaveId, SchedulingError, Slot,
    },
};

use super::SaveCoachLeaveError;
use crate::Unavailable;

/// コーチのお休み（[`CoachLeave`]）の保存先のポート。お休みは予約と枠をまとめて変えるので、予約の保存先
/// （[`super::BookingRepository`]）と同じ実装が持つ（infra の `PgBookingRepository` と
/// [`crate::in_memory::InMemoryBookingRepository`]）。
///
/// 不変条件の判定はクロージャ（domain の [`CoachLeave::start`]）だけが行い、保存先は判定しない。その代わり、読み取りから
/// 保存までを 1 つにまとめ（全部か何もしないか）、同じコーチのお休みと、そのコーチの期間の枠への予約の作成・確定・
/// キャンセルと直列にする（確かめた後に入った予約を見落としてキャンセルしないことがないように）。
pub trait CoachLeaveRepository: Send + Sync {
    /// 新しいお休みの記録の ID を払い出す。
    fn next_coach_leave_id(&self) -> CoachLeaveId;

    /// `coach_id` のコーチの、開始が `period` に入る枠（状態を問わない）と、それらの枠への予約（状態を問わない）。読むだけで
    /// 何も変えない。本部が見る期間の予約と枠の確かめ（[`super::PreviewCoachLeave`]）が読む。
    fn coach_slots_starting_in(
        &self,
        coach_id: AccountId,
        period: BookingPeriod,
    ) -> impl Future<Output = Result<(Vec<Slot>, Vec<Booking>), Unavailable>> + Send;

    /// `coach_id` のコーチの、開始が `period` に入る枠とそれらの枠への予約（[`Self::coach_slots_starting_in`] と同じ）と、
    /// そのコーチのお休みの記録を読み、`start` に渡す。`start` が `Ok` なら、変えた予約（キャンセル）と枠（取り下げ）と、
    /// 返したお休みの記録を 1 つにまとめて保存し、保存した記録を返す。`start` が `Err` なら何も保存しない。
    fn start_coach_leave<F>(
        &self,
        coach_id: AccountId,
        period: BookingPeriod,
        start: F,
    ) -> impl Future<Output = Result<CoachLeave, SaveCoachLeaveError>> + Send
    where
        F: FnOnce(
                &mut [Slot],
                &mut [Booking],
                &[CoachLeave],
            ) -> Result<CoachLeave, SchedulingError>
            + Send;

    /// `booking_ids` のどれかをキャンセルしたお休みの記録。ない予約・お休みでキャンセルしていない予約は結果に寄与しない。
    /// 並びは決めない。読むだけで何も変えない。生徒のお知らせの本文に日時と一言を添えるとき
    /// （[`super::SchedulingCoachLeaveNoticeLookup`]）に読む。
    fn coach_leaves_of_bookings(
        &self,
        booking_ids: &BTreeSet<BookingId>,
    ) -> impl Future<Output = Result<Vec<CoachLeave>, Unavailable>> + Send;
}
