use std::time::SystemTime;

use domain::{catalog::Sport, identity::AccountId, scheduling::Booking};

use super::SlotCursor;

/// 保存先に渡す検索条件。公開中（[`domain::scheduling::SlotStatus::Open`]）で残りの席がある
/// （[`domain::scheduling::Slot::remaining_seats`] が 1 以上。未確定の予約で埋まった枠も除く）枠のうち、
/// 次の条件をすべて満たすものを（開始時刻, スロット ID）の昇順で最大 `limit` 件。受付を止めているコーチ
/// （[`domain::scheduling::CoachAcceptance`] が停止中）の枠は条件によらず除く（除くのは保存先。[`Self::matches`] は見ない）。
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
    /// あればこの生徒が有効な予約（未確定か確定）を持つ枠を除く（呼んだ生徒が押さえている枠）
    pub held_by: Option<AccountId>,
    pub limit: usize,
}

impl SlotCriteria {
    /// 枠が条件（件数を除く）を満たすか。`bookings` は予約（ほかの枠の予約が混じってもよい。
    /// [`Self::held_by`] の判定に使う）。インメモリの実装とテストが使う。
    pub fn matches(&self, slot: &domain::scheduling::Slot, bookings: &[Booking]) -> bool {
        slot.status() == domain::scheduling::SlotStatus::Open
            && slot.remaining_seats() > 0
            && slot.sport() == self.sport
            && self.coach_id.is_none_or(|id| slot.coach_id() == id)
            && slot.starts_at() >= self.starts_after
            && self
                .starts_before
                .is_none_or(|before| slot.starts_at() < before)
            && self
                .after
                .is_none_or(|c| (slot.starts_at(), slot.id()) > (c.starts_at, c.slot_id))
            && self.held_by.is_none_or(|student_id| {
                !bookings.iter().any(|b| {
                    b.slot_id() == slot.id()
                        && b.student_id() == student_id
                        && b.status().is_active()
                })
            })
    }
}
