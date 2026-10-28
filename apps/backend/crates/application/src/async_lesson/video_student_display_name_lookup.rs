use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::{identity::AccountId, profile::DisplayName};

use crate::Unavailable;

/// async-lesson が profile に生徒の表示名を問い合わせるポート（コーチに届いた動画の一覧で、動画を送った生徒の
/// 表示名を添える）。
///
/// 表示名の持ち主は profile で、async-lesson は読むだけ。実装は
/// [`crate::profile::ProfileStudentDisplayNameLookup`]。
pub trait VideoStudentDisplayNameLookup: Send + Sync {
    /// `student_ids` の生徒の表示名。生徒のアカウントがない ID は結果に含めない。
    fn find_student_display_names(
        &self,
        student_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, DisplayName>, Unavailable>> + Send;
}
