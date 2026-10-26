//! 行動イベント（specs/analytics/events.yaml）を発火するポート。
//!
//! Gen1 の行動トラッキングはモック。発火の失敗でユースケースを失敗させないので、戻り値はない。

use std::{future::Future, time::SystemTime};

use domain::{
    identity::{AccountId, Sport},
    scheduling::SlotId,
};

/// 行動イベント。変種は events.yaml の `name` に 1 対 1 で対応し、属性は `required` と同じ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalyticsEvent {
    /// `account.registered`
    AccountRegistered {
        student_id: AccountId,
        sport: Sport,
        occurred_at: SystemTime,
    },
    /// `slot.published`（KPI「コーチの予約枠充足率」の入力）
    SlotPublished {
        coach_id: AccountId,
        slot_id: SlotId,
        starts_at: SystemTime,
        occurred_at: SystemTime,
    },
}

impl AnalyticsEvent {
    /// events.yaml の `name`。
    pub const fn name(&self) -> &'static str {
        match self {
            AnalyticsEvent::AccountRegistered { .. } => "account.registered",
            AnalyticsEvent::SlotPublished { .. } => "slot.published",
        }
    }
}

pub trait AnalyticsSink: Send + Sync {
    fn emit(&self, event: AnalyticsEvent) -> impl Future<Output = ()> + Send;
}
