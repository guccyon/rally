use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::{identity::AccountId, profile::DisplayName};

use crate::Unavailable;

/// identity が profile にコーチの表示名を問い合わせるポート（本部が見る生徒 1 人の利用の様子で、予約とレッスンの
/// コーチの表示名を添える）。実装は [`crate::profile::ProfileCoachDisplayNameLookup`]。
pub trait StudentActivityCoachDisplayNameLookup: Send + Sync {
    /// `coach_ids` のコーチそれぞれの表示名。表示名が引けないコーチ（コーチでないアカウント・ないアカウント）は結果に含めない。
    fn find_coach_display_names(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, DisplayName>, Unavailable>> + Send;
}
