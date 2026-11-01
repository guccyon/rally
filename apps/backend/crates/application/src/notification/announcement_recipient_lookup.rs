use std::future::Future;

use domain::{catalog::Sport, identity::AccountId};

use crate::Unavailable;

/// notification が本部からのお知らせを受け取る生徒を問い合わせるポート。
///
/// 生徒のアカウントとスポーツの持ち主は identity / profile（データは当面 identity の表）で、notification は読むだけ。
/// 実装は [`crate::profile::ProfileAnnouncementRecipientLookup`]。
pub trait AnnouncementRecipientLookup: Send + Sync {
    /// スポーツが `sport` の生徒すべて（`None` なら生徒全員。ゴルフの生徒も含む）。並びは問わない。
    fn find_students(
        &self,
        sport: Option<Sport>,
    ) -> impl Future<Output = Result<Vec<AccountId>, Unavailable>> + Send;
}
