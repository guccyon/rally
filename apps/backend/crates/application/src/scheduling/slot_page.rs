use std::collections::BTreeMap;

use domain::{identity::AccountId, scheduling::Slot};

use super::{CoachIntroduction, SlotCursor};

/// スロットの検索の 1 ページ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotPage {
    pub items: Vec<Slot>,
    /// 次のページの位置。次のページがなければ `None`
    pub next_cursor: Option<SlotCursor>,
    /// `items` のコーチの紹介（コーチの ID ごと）。紹介が引けないコーチは含まない
    pub coaches: BTreeMap<AccountId, CoachIntroduction>,
}
