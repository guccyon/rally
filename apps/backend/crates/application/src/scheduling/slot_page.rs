use domain::scheduling::Slot;

use super::SlotCursor;

/// スロットの検索の 1 ページ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotPage {
    pub items: Vec<Slot>,
    /// 次のページの位置。次のページがなければ `None`
    pub next_cursor: Option<SlotCursor>,
}
