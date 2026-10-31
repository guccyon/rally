use std::{collections::BTreeSet, future::Future};

use domain::identity::AccountId;

use crate::Unavailable;

/// matching が scheduling に、受付を止めているコーチを問い合わせるポート（停止中のコーチはコーチを探す一覧に出さない）。
///
/// 受付の状態の持ち主は scheduling（domain の [`domain::scheduling::CoachAcceptance`]）で、matching は読むだけ。実装は
/// [`crate::scheduling::SchedulingCoachAcceptanceLookup`]。
pub trait SuspendedCoachLookup: Send + Sync {
    /// 受付を止めているコーチ（停止中）の ID。
    fn find_suspended_coaches(
        &self,
    ) -> impl Future<Output = Result<BTreeSet<AccountId>, Unavailable>> + Send;
}
