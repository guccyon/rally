use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::identity::{Account, AccountId};

use crate::Unavailable;

/// live-lesson が profile に通話セッションのコーチのアカウントを問い合わせるポート（本部が見る、終わっていないまま時間の
/// 過ぎたセッションの一覧を競技で絞るときに、コーチの提供スポーツを見る）。
///
/// コーチの提供スポーツの持ち主は profile で、live-lesson は読むだけ。実装は [`crate::profile::ProfileCoachAccountLookup`]。
pub trait SessionCoachAccountLookup: Send + Sync {
    /// `coach_ids` のコーチのアカウント。コーチのアカウントがない ID は結果に含めない。
    fn find_coach_accounts(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, Account>, Unavailable>> + Send;
}
