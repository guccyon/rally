use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::{identity::AccountId, profile::DisplayName};

use crate::Unavailable;

/// scheduling が profile にコーチの表示名を問い合わせるポート（本部が見る予約の一覧で予約に、生徒の次の操作
/// （[`super::GetMyNextActions`]）で予約・通話・動画にコーチの表示名を添える）。
///
/// 表示名の持ち主は profile で、scheduling は読むだけ。実装は [`crate::profile::ProfileCoachDisplayNameLookup`]。
pub trait BookingCoachDisplayNameLookup: Send + Sync {
    /// `coach_ids` のコーチの表示名。コーチのアカウントがない ID は結果に含めない。
    fn find_coach_display_names(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, DisplayName>, Unavailable>> + Send;
}
