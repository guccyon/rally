use std::time::SystemTime;

/// 合成する予約枠 1 つ。同じコーチの枠は重ならない（[`crate::SeedPlan`] が時間を割り当てる）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotPlan {
    /// [`crate::SeedPlan::coaches`] の添字
    pub coach: usize,
    pub starts_at: SystemTime,
    pub ends_at: SystemTime,
}
