use super::HeadquartersUnpublishReason;

/// 本部がレッスンメニューにした操作（[`super::LessonMenuPublicationChange`] の中身）。非公開にした（そのとき書いた理由
/// つき。空なら書かなかった）か、公開に戻したか。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LessonMenuPublicationAction {
    /// 本部が非公開にした（[`super::LessonMenu::unpublish_by_headquarters`]）
    Unpublished { reason: HeadquartersUnpublishReason },
    /// 本部が公開に戻した（[`super::LessonMenu::republish_by_headquarters`]）
    Republished,
}
