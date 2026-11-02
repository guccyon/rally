use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::{identity::AccountId, profile::DisplayName};

use crate::Unavailable;

/// live-lesson が profile に通話セッションの生徒の表示名を問い合わせるポート（本部が見る、終わっていないまま時間の
/// 過ぎたセッションの一覧で、セッションを開始した予約の生徒の表示名を添える）。
///
/// 表示名の持ち主は profile で、live-lesson は読むだけ。実装は [`crate::profile::ProfileStudentDisplayNameLookup`]。
pub trait SessionStudentDisplayNameLookup: Send + Sync {
    /// `student_ids` の生徒の表示名。生徒のアカウントがない ID は結果に含めない。
    fn find_student_display_names(
        &self,
        student_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, DisplayName>, Unavailable>> + Send;
}
