use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::{identity::AccountId, profile::DisplayName};

use crate::Unavailable;

/// 完了記録が profile にコーチの表示名を問い合わせるポート（生徒が受けたレッスンの一覧で、レッスンのコーチの表示名を
/// 添える）。
///
/// 表示名の持ち主は profile で、完了記録は読むだけ。実装は [`crate::profile::ProfileCoachDisplayNameLookup`]。
pub trait LessonCompletionCoachDisplayNameLookup: Send + Sync {
    /// `coach_ids` のコーチの表示名。コーチのアカウントがない ID は結果に含めない。
    fn find_coach_display_names(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, DisplayName>, Unavailable>> + Send;
}
