use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::{identity::AccountId, profile::DisplayName};

use crate::Unavailable;

/// messaging が profile にスレッドのコーチの表示名を問い合わせるポート（本部が見る、返信のないまま時間の過ぎた
/// スレッドの一覧で、スレッドに担当コーチの表示名を添える）。
///
/// 表示名の持ち主は profile で、messaging は読むだけ。実装は [`crate::profile::ProfileCoachDisplayNameLookup`]。
pub trait ThreadCoachDisplayNameLookup: Send + Sync {
    /// `coach_ids` のコーチの表示名。コーチのアカウントがない ID は結果に含めない。
    fn find_coach_display_names(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, DisplayName>, Unavailable>> + Send;
}
