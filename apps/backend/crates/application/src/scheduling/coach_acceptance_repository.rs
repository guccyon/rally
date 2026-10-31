use std::{collections::BTreeSet, future::Future};

use domain::{
    identity::AccountId,
    scheduling::{CoachAcceptance, SchedulingError},
};

use super::UpdateCoachAcceptanceError;
use crate::Unavailable;

/// コーチの受付（[`CoachAcceptance`]）の保存先のポート。実装は infra（Postgres の `scheduling.coach_acceptances`）と、
/// テスト・ローカル起動用の [`crate::in_memory::InMemoryCoachAcceptanceRepository`]。
///
/// 一度も止めていないコーチの受付は保存していなくても受付中として扱う。枠の検索（[`super::SlotRepository::search`]）と
/// 予約の作成（[`super::BookingRepository::find_coach_acceptance`]）も同じ状態を読む。
pub trait CoachAcceptanceRepository: Send + Sync {
    /// `coach_id` のコーチの受付（保存していなければ受付中）を読み、`update` に渡す。`update` が `Ok` なら保存し、
    /// 保存した受付を返す。コーチがいるかは確かめない（呼ぶ側が確かめる）。
    ///
    /// 判定は `update`（domain の [`CoachAcceptance`]）だけが行い、保存先は判定しない。その代わり、同じコーチへの
    /// 呼び出しは読み取りから保存までを直列にする（並行な停止が両方通ることがないように）。
    fn update<F>(
        &self,
        coach_id: AccountId,
        update: F,
    ) -> impl Future<Output = Result<CoachAcceptance, UpdateCoachAcceptanceError>> + Send
    where
        F: FnOnce(&mut CoachAcceptance) -> Result<(), SchedulingError> + Send;

    /// 受付を止めているコーチ（停止中）の ID。読むだけで何も変えない。
    fn suspended_coaches(
        &self,
    ) -> impl Future<Output = Result<BTreeSet<AccountId>, Unavailable>> + Send;
}
