use std::future::Future;

use domain::{catalog::Sport, identity::AccountId};

use crate::Unavailable;

/// matching が profile に生徒のスポーツを問い合わせるポート（コーチを探す一覧でスポーツを省いたときに使う）。
///
/// 生徒のスポーツの持ち主は profile（データは当面 identity の表）で、matching は読むだけ。実装は
/// [`crate::profile::ProfileStudentSportLookup`]。
pub trait StudentSportLookup: Send + Sync {
    /// 生徒 `student_id` のスポーツ。生徒のアカウントがなければ `None`。
    fn find_student_sport(
        &self,
        student_id: AccountId,
    ) -> impl Future<Output = Result<Option<Sport>, Unavailable>> + Send;
}
