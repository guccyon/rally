use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::{identity::AccountId, profile::DisplayName};

use crate::Unavailable;

/// review が profile に評価した生徒の表示名を問い合わせるポート（本部が見る低い評価の一覧で、評価に生徒の
/// 表示名を添える）。
///
/// 表示名の持ち主は profile で、review は読むだけ。実装は [`crate::profile::ProfileStudentDisplayNameLookup`]。
pub trait ReviewerDisplayNameLookup: Send + Sync {
    /// `student_ids` の生徒の表示名。生徒のアカウントがない ID は結果に含めない。
    fn find_student_display_names(
        &self,
        student_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, DisplayName>, Unavailable>> + Send;
}
