use std::future::Future;

use domain::{
    DomainError,
    catalog::{LessonMenu, LessonMenuId, LessonMenuPublicationChange},
    identity::AccountId,
};

use super::ChangeLessonMenuPublicationError;
use crate::Unavailable;

/// レッスンメニューの保存先のポート。実装はテスト・ローカル起動用の
/// [`crate::in_memory::InMemoryLessonMenuRepository`] と、infra の Postgres（`catalog.lesson_menus`）。
///
/// 不変条件の判定（コーチの提供スポーツか、メニューのコーチか）は domain の [`LessonMenu`] が行う。
/// 同じメニューへの並行な変更は後から保存した方が残る（本部の非公開・公開に戻す操作 [`Self::change_publication`] は除く）。
/// 本部が非公開にした・公開に戻した記録（[`LessonMenuPublicationChange`]）も同じ保存先が持つ。
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

    /// ID のメニューを読み、`change` に渡す（本部の非公開・公開に戻す操作）。`change` が `Ok` ならメニューと、`change` が
    /// 返した記録を同じ保存の中で（どちらか片方だけにならないように）保存し、保存したメニューを返す。
    ///
    /// メニューがなければ [`ChangeLessonMenuPublicationError::LessonMenuNotFound`]、`change` の違反は
    /// [`ChangeLessonMenuPublicationError::Invalid`] で、どちらも何も保存しない。判定は `change`（domain の [`LessonMenu`]）
    /// だけが行う。同じメニューへの呼び出しは読み取りから保存までを直列にする（並行な非公開が両方通って記録が 2 件に
    /// ならないように）。
    fn change_publication<F>(
        &self,
        id: LessonMenuId,
        change: F,
    ) -> impl Future<Output = Result<LessonMenu, ChangeLessonMenuPublicationError>> + Send
    where
        F: FnOnce(&mut LessonMenu) -> Result<LessonMenuPublicationChange, DomainError> + Send;

    /// ID のメニューを本部が非公開にした・公開に戻した記録を、新しい順（変えた時刻の新しい順。同じ時刻は後に保存した順）に
    /// 返す。記録がなければ空。メニューがあるかは確かめない。読むだけで何も変えない。
    fn publication_changes(
        &self,
        id: LessonMenuId,
    ) -> impl Future<Output = Result<Vec<LessonMenuPublicationChange>, Unavailable>> + Send;
}
