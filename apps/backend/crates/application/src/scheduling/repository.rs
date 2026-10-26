use std::future::Future;

use domain::{
    DomainError,
    scheduling::{CoachCalendar, Slot, SlotId},
};

use super::SlotCriteria;
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
        F: FnOnce(&mut CoachCalendar, Slot) -> Result<(), DomainError> + Send;

    /// 条件に合う枠（[`SlotCriteria`] の順で最大 `limit` 件）。
    fn search(
        &self,
        criteria: &SlotCriteria,
    ) -> impl Future<Output = Result<Vec<Slot>, Unavailable>> + Send;
}

#[derive(Debug)]
pub enum AddToCalendarError {
    /// カレンダーが枠を受け付けない（`add` が返した不変条件の違反）
    Rejected(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}
