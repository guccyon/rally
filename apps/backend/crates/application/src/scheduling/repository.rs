use std::future::Future;

use domain::{
    identity::AccountId,
    scheduling::{CoachCalendar, SchedulingError, Slot, SlotId},
};

use super::{SlotCriteria, UpdateSlotError};
use crate::Unavailable;

/// スロットの保存先のポート。実装は infra（Postgres の `scheduling.slots`）と、
/// テスト・ローカル起動用の [`crate::in_memory::InMemorySlotRepository`]。
pub trait SlotRepository: Send + Sync {
    /// 新しいスロットの ID を払い出す。
    fn next_id(&self) -> SlotId;

    /// `slot` のコーチのカレンダー（少なくとも `slot` の期間に重なる時間を占める枠を含む）を読み、
    /// `add` に渡す。`add` が `Ok` なら `slot` を保存する。
    ///
    /// 重なりの判定は `add`（domain の [`CoachCalendar`]）だけが行い、保存先は判定しない。
    /// その代わり、同じコーチへの呼び出しは読み取りから保存までを直列にする（並行に公開された
    /// 2 つの枠が互いを見ずに両方保存されることがないように）。
    fn add_to_calendar<F>(
        &self,
        slot: Slot,
        add: F,
    ) -> impl Future<Output = Result<(), AddToCalendarError>> + Send
    where
        F: FnOnce(&mut CoachCalendar, Slot) -> Result<(), SchedulingError> + Send;

    /// `slot_id` のスロット（状態を問わない。席は予約から数える）を読み、`update` に渡す。`update` が `Ok` なら
    /// スロットの状態を保存し、保存したスロットを返す。席の数は保存しない（予約から数える）。
    ///
    /// 不変条件の判定は `update`（domain の [`Slot`]）だけが行い、保存先は判定しない。その代わり、同じスロットへの
    /// 予約の作成・確定・キャンセル（[`super::BookingRepository`]）と読み取りから保存までを直列にする（取り下げと
    /// 並行な予約が互いを見落として、予約の入った枠を閉じることがないように）。
    fn update<F>(
        &self,
        slot_id: SlotId,
        update: F,
    ) -> impl Future<Output = Result<Slot, UpdateSlotError>> + Send
    where
        F: FnOnce(&mut Slot) -> Result<(), SchedulingError> + Send;

    /// 条件に合う枠（[`SlotCriteria`] の順で最大 `limit` 件）。受付を止めているコーチの枠は除く（除いた後で `limit` 件）。
    fn search(
        &self,
        criteria: &SlotCriteria,
    ) -> impl Future<Output = Result<Vec<Slot>, Unavailable>> + Send;

    /// `coach_id` のコーチの枠（状態を問わない。終わった枠も含む）。並びは決めない。読むだけで何も変えない。
    /// コーチの自分の枠の一覧（[`super::ListCoachSlots`]）が読む。
    fn slots_of_coach(
        &self,
        coach_id: AccountId,
    ) -> impl Future<Output = Result<Vec<Slot>, Unavailable>> + Send;
}

#[derive(Debug)]
pub enum AddToCalendarError {
    /// カレンダーが枠を受け付けない（`add` が返した不変条件の違反）
    Rejected(SchedulingError),
    /// 保存先の障害
    Unavailable(Unavailable),
}
