use std::future::Future;

use domain::{
    catalog::{LessonMenu, LessonMenuId},
    identity::AccountId,
};

use crate::Unavailable;

/// レッスンメニューの保存先のポート。実装はテスト・ローカル起動用の
/// [`crate::in_memory::InMemoryLessonMenuRepository`] と、infra の Postgres（`catalog.lesson_menus`）。
///
/// 不変条件の判定（コーチの提供スポーツか、メニューのコーチか）は domain の [`LessonMenu`] が行う。
/// 同じメニューへの並行な変更は後から保存した方が残る。
pub trait LessonMenuRepository: Send + Sync {
    /// 新しいメニューの ID を払い出す。
    fn next_id(&self) -> LessonMenuId;

    /// 新しいメニューを保存する。
    fn insert(&self, menu: LessonMenu) -> impl Future<Output = Result<(), Unavailable>> + Send;

    /// ID のメニュー。なければ `None`。
    fn find(
        &self,
        id: LessonMenuId,
    ) -> impl Future<Output = Result<Option<LessonMenu>, Unavailable>> + Send;

    /// 既存のメニューを `menu` で置き換える（ID で探す。コーチと作った時刻は変えない）。
    fn update(&self, menu: LessonMenu) -> impl Future<Output = Result<(), Unavailable>> + Send;

    /// コーチ `coach_id` のメニューを、公開中かを問わずすべて読む（絞らない・並べない。並べるのはユースケース）。
    fn find_by_coach(
        &self,
        coach_id: AccountId,
    ) -> impl Future<Output = Result<Vec<LessonMenu>, Unavailable>> + Send;

    /// すべてのコーチのメニューを、公開中かを問わずすべて読む（絞らない・並べない。絞るのと並べるのはユースケース）。
    fn find_all(&self) -> impl Future<Output = Result<Vec<LessonMenu>, Unavailable>> + Send;
}
