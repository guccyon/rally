use std::future::Future;

use domain::{
    catalog::{LessonMenu, LessonMenuId},
    identity::AccountId,
};

use crate::Unavailable;

/// レッスンメニューの保存先のポート。実装はテスト・ローカル起動用の
/// [`crate::in_memory::InMemoryLessonMenuRepository`] と、infra の Postgres（`catalog.lesson_menus`）。
///
/// 不変条件の判定は domain の [`LessonMenu`] が行う。メニューは消さない（非公開にするだけ）。
pub trait LessonMenuRepository: Send + Sync {
    /// 新しいメニューの ID を払い出す。
    fn next_id(&self) -> LessonMenuId;

    /// 新しいメニューを保存する。
    fn insert(&self, menu: LessonMenu) -> impl Future<Output = Result<(), Unavailable>> + Send;

    /// `id` のメニュー。なければ `None`。
    fn find(
        &self,
        id: LessonMenuId,
    ) -> impl Future<Output = Result<Option<LessonMenu>, Unavailable>> + Send;

    /// 保存してあるメニューを `menu` で置き換える（同じメニューの並行な変更は後に保存した方が残る）。
    fn update(&self, menu: LessonMenu) -> impl Future<Output = Result<(), Unavailable>> + Send;

    /// コーチ `coach_id` のメニューを、公開中か非公開かを問わずすべて読む（並べない。並べるのは一覧のユースケース）。
    fn find_by_coach(
        &self,
        coach_id: AccountId,
    ) -> impl Future<Output = Result<Vec<LessonMenu>, Unavailable>> + Send;
}
