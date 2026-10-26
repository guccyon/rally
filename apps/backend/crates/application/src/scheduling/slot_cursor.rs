use std::time::SystemTime;

use domain::scheduling::{Slot, SlotId};

/// スロットの検索のページの位置。検索結果は（開始時刻, スロット ID）の昇順で、カーソルより
/// 後ろの枠が次のページになる。外部での文字列表現は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotCursor {
    pub starts_at: SystemTime,
    pub slot_id: SlotId,
}

impl SlotCursor {
    /// `slot` の直後から始まる位置。
    pub fn after(slot: &Slot) -> Self {
        Self {
            starts_at: slot.starts_at(),
            slot_id: slot.id(),
        }
    }
}
