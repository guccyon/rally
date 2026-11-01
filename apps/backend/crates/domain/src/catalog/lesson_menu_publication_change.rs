use std::time::SystemTime;

use super::{LessonMenuId, LessonMenuPublicationAction};
use crate::identity::AccountId;

/// 本部がレッスンメニューを非公開にした・公開に戻した記録 1 件（glossary の lesson_menu_publication_change）。どのメニューを、
/// 誰（本部のアカウント）が、いつ、どうしたか（[`LessonMenuPublicationAction`]）を持つ。
///
/// 作るのは [`super::LessonMenu::unpublish_by_headquarters`]・[`super::LessonMenu::republish_by_headquarters`] がメニューを
/// 変えたときだけ（違反で何も変えなかったときは作らない）。メニューの今の状態（[`super::HeadquartersUnpublish`]）は公開に
/// 戻すと消えるが、この記録は残る。保存した記録を戻すときは [`Self::restore`]。記録は後から書き換えない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonMenuPublicationChange {
    lesson_menu_id: LessonMenuId,
    action: LessonMenuPublicationAction,
    changed_by: AccountId,
    changed_at: SystemTime,
}

impl LessonMenuPublicationChange {
    /// 保存した記録から戻す。
    pub const fn restore(
        lesson_menu_id: LessonMenuId,
        action: LessonMenuPublicationAction,
        changed_by: AccountId,
        changed_at: SystemTime,
    ) -> Self {
        Self {
            lesson_menu_id,
            action,
            changed_by,
            changed_at,
        }
    }

    pub const fn lesson_menu_id(&self) -> LessonMenuId {
        self.lesson_menu_id
    }

    /// 非公開にした（理由つき）か、公開に戻したか。
    pub const fn action(&self) -> &LessonMenuPublicationAction {
        &self.action
    }

    /// 非公開にした・公開に戻した本部のアカウントの ID。
    pub const fn changed_by(&self) -> AccountId {
        self.changed_by
    }

    pub const fn changed_at(&self) -> SystemTime {
        self.changed_at
    }
}
