use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::{identity::AccountId, profile::DisplayName};

use crate::Unavailable;

/// messaging が profile にスレッドの生徒の表示名を問い合わせるポート（本部が見る、返信のないまま時間の過ぎた
/// スレッドの一覧で、スレッドに生徒の表示名を添える）。
///
/// 表示名の持ち主は profile で、messaging は読むだけ。実装は [`crate::profile::ProfileStudentDisplayNameLookup`]。
pub trait ThreadStudentDisplayNameLookup: Send + Sync {
    /// `student_ids` の生徒の表示名。生徒のアカウントがない ID は結果に含めない。
    fn find_student_display_names(
        &self,
        student_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, DisplayName>, Unavailable>> + Send;
}
