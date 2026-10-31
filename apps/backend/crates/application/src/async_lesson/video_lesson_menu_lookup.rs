use std::future::Future;

use domain::catalog::{LessonMenu, LessonMenuId};

use crate::Unavailable;

/// async-lesson が catalog にレッスンメニューを問い合わせるポート（動画の送付で生徒が選んだメニューを確かめ、
/// 送った時点の ID・名前を動画に写す）。
///
/// メニューの持ち主は catalog で、async-lesson は読むだけ（動画が持つのは送った時点の写しだけ）。実装は
/// [`crate::catalog::CatalogLessonMenuLookup`]。
pub trait VideoLessonMenuLookup: Send + Sync {
    /// `lesson_menu_id` のメニュー（公開中かを問わない）。なければ `None`。
    fn find_lesson_menu(
        &self,
        lesson_menu_id: LessonMenuId,
    ) -> impl Future<Output = Result<Option<LessonMenu>, Unavailable>> + Send;
}
