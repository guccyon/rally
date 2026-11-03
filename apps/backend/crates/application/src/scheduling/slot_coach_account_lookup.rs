use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::identity::{Account, AccountId};

use crate::Unavailable;

/// scheduling が profile に枠のコーチのアカウントを問い合わせるポート（本部が見る予約枠の需給と予約の一覧を競技で
/// 絞るときに、コーチの提供スポーツを見る）。
///
/// コーチの提供スポーツの持ち主は profile で、scheduling は読むだけ。実装は [`crate::profile::ProfileCoachAccountLookup`]。
pub trait SlotCoachAccountLookup: Send + Sync {
    /// `coach_ids` のコーチのアカウント。コーチのアカウントがない ID は結果に含めない。
    fn find_slot_coach_accounts(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, Account>, Unavailable>> + Send;
}
