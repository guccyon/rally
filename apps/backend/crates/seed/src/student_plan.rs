use std::time::SystemTime;

use crate::{Retention, Step};

/// 合成する生徒 1 人と、その履歴。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudentPlan {
    /// 担当のコーチ（[`crate::SeedPlan::coaches`] の添字）。生徒のスポーツはコーチのスポーツ
    pub coach: usize,
    pub registered_at: SystemTime,
    pub retention: Retention,
    /// 時刻の順に並んだ履歴（最初は [`Step::Register`]）
    pub steps: Vec<Step>,
}
