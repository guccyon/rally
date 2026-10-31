use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::catalog::{LessonMenu, LessonMenuId};

use crate::Unavailable;

/// scheduling が catalog にレッスンメニューを問い合わせるポート（枠の公開で結びつけるメニューを確かめ、枠の一覧に
/// 結びついたメニューの名前と時間を添える）。
///
/// メニューの持ち主は catalog で、scheduling は読むだけ（枠が持つのはメニューの ID だけ）。実装は
/// [`crate::catalog::CatalogLessonMenuLookup`]。
pub trait LessonMenuLookup: Send + Sync {
    /// `lesson_menu_ids` のメニュー（公開中かを問わない）。ないメニューは結果に含めない。
    fn find_lesson_menus(
        &self,
        lesson_menu_ids: &BTreeSet<LessonMenuId>,
    ) -> impl Future<Output = Result<BTreeMap<LessonMenuId, LessonMenu>, Unavailable>> + Send;
}
