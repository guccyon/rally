use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::{identity::AccountId, profile::DisplayName};

use crate::Unavailable;

/// scheduling が profile に生徒の表示名を問い合わせるポート（コーチの自分の予約の一覧で、予約に生徒の表示名を添える）。
///
/// 表示名の持ち主は profile で、scheduling は読むだけ。実装は
/// [`crate::profile::ProfileStudentDisplayNameLookup`]。
pub trait StudentDisplayNameLookup: Send + Sync {
    /// `student_ids` の生徒の表示名。生徒のアカウントがない ID は結果に含めない。
    fn find_student_display_names(
        &self,
        student_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, DisplayName>, Unavailable>> + Send;
}
