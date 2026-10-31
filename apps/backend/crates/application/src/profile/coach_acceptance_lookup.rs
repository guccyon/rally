use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::{identity::AccountId, scheduling::CoachAcceptanceStatus};

use crate::Unavailable;

/// profile が scheduling にコーチの受付の状態を問い合わせるポート（本部が見るコーチの一覧に、1 件ごとに受付の状態を添える）。
///
/// 受付の状態の持ち主は scheduling（domain の [`domain::scheduling::CoachAcceptance`]）で、profile は読むだけ。実装は
/// [`crate::scheduling::SchedulingCoachAcceptanceLookup`]。
pub trait CoachAcceptanceLookup: Send + Sync {
    /// `coach_ids` のコーチの受付の状態。どの ID も結果に含める（止めていないコーチは受付中）。
    fn find_coach_acceptances(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, CoachAcceptanceStatus>, Unavailable>> + Send;
}
