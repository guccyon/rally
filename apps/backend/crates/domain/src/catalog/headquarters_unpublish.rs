use std::time::SystemTime;

use super::HeadquartersUnpublishReason;

/// 本部によるレッスンメニューの非公開（glossary の lesson_menu_headquarters_unpublish）。本部が非公開にした時刻と、
/// そのとき書いた理由（空なら書かなかった）。
///
/// これを持つメニューは非公開で、コーチは自分で公開し直せない（[`super::LessonMenu::revise`]）。公開に戻すのは本部だけ
/// （[`super::LessonMenu::republish_by_headquarters`]）で、戻すと消える（履歴は残さない）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadquartersUnpublish {
    reason: HeadquartersUnpublishReason,
    unpublished_at: SystemTime,
}

impl HeadquartersUnpublish {
    pub const fn new(reason: HeadquartersUnpublishReason, unpublished_at: SystemTime) -> Self {
        Self {
            reason,
            unpublished_at,
        }
    }

    pub const fn reason(&self) -> &HeadquartersUnpublishReason {
        &self.reason
    }

    /// 本部が非公開にした時刻。
    pub const fn unpublished_at(&self) -> SystemTime {
        self.unpublished_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_reason_and_time() {
        let unpublish = HeadquartersUnpublish::new(
            HeadquartersUnpublishReason::new("誤った料金の記載").unwrap(),
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(unpublish.reason().as_str(), "誤った料金の記載");
        assert_eq!(unpublish.unpublished_at(), SystemTime::UNIX_EPOCH);
    }
}
