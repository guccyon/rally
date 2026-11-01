use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::{identity::AccountId, profile::DisplayName};

use crate::Unavailable;

/// live-lesson が profile に通話セッションのコーチの表示名を問い合わせるポート（本部が見る、終わっていないまま時間の
/// 過ぎたセッションの一覧で、セッションにコーチの表示名を添える）。
///
/// 表示名の持ち主は profile で、live-lesson は読むだけ。実装は [`crate::profile::ProfileCoachDisplayNameLookup`]。
pub trait SessionCoachDisplayNameLookup: Send + Sync {
    /// `coach_ids` のコーチの表示名。コーチのアカウントがない ID は結果に含めない。
    fn find_coach_display_names(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, DisplayName>, Unavailable>> + Send;
}
