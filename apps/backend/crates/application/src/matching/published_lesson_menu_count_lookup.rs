use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::{catalog::Sport, identity::AccountId};

use crate::Unavailable;

/// matching が catalog にコーチごとの公開中のレッスンメニューの数を問い合わせるポート。
///
/// レッスンメニューの持ち主は catalog で、matching は読むだけ。実装は
/// [`crate::catalog::CatalogPublishedLessonMenuCountLookup`]。
pub trait PublishedLessonMenuCountLookup: Send + Sync {
    /// `coach_ids` のコーチの、スポーツ `sport` の公開中のメニューの数。メニューのないコーチは結果に含めない。
    fn count_published_lesson_menus(
        &self,
        coach_ids: &BTreeSet<AccountId>,
        sport: Sport,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, u32>, Unavailable>> + Send;
}
