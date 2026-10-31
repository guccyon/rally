use std::time::SystemTime;

use domain::{catalog::Sport, identity::AccountId};

use super::SlotCursor;

/// 保存先に渡す検索条件。公開中（[`domain::scheduling::SlotStatus::Open`]）の枠のうち、
/// 次の条件をすべて満たすものを（開始時刻, スロット ID）の昇順で最大 `limit` 件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotCriteria {
    pub sport: Sport,
    /// あればこのコーチの枠だけ
    pub coach_id: Option<AccountId>,
    /// この時刻以降に始まる（含む）
    pub starts_after: SystemTime,
    /// あればこの時刻より前に始まる（含まない）
    pub starts_before: Option<SystemTime>,
    /// あればこの位置より後ろ
    pub after: Option<SlotCursor>,
    pub limit: usize,
}

impl SlotCriteria {
    /// 枠が条件（件数を除く）を満たすか。インメモリの実装とテストが使う。
    pub fn matches(&self, slot: &domain::scheduling::Slot) -> bool {
        slot.status() == domain::scheduling::SlotStatus::Open
            && slot.sport() == self.sport
            && self.coach_id.is_none_or(|id| slot.coach_id() == id)
            && slot.starts_at() >= self.starts_after
            && self
                .starts_before
                .is_none_or(|before| slot.starts_at() < before)
            && self
                .after
                .is_none_or(|c| (slot.starts_at(), slot.id()) > (c.starts_at, c.slot_id))
    }
}
