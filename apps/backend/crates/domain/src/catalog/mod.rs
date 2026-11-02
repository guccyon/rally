//! catalog: カタログ（specs/context-map.yaml の catalog）。コーチとレッスン内容のセットが商品（docs/adr/0009）。
//!
//! 提供スポーツの種類 [`Sport`]（identity の生徒・コーチもこれを参照する）と、
//! コーチのレッスンメニュー（glossary の lesson「レッスン種別」。スポーツごとにコーチが提供するメニューで、
//! 同期と非同期の別を持つ）を集約 [`LessonMenu`] で持つ。契約の `Lesson`（student-app の完了したレッスン）と
//! [`crate::lesson`]（完了の記録）と区別するため、型の名前は「レッスンメニュー」にした。
//! メニューを作る・直す・非公開にするのはそのコーチ（[`LessonMenu::create`] / [`LessonMenu::revise`]）。本部はコーチに代わって
//! 非公開にし・公開に戻せる（[`LessonMenu::unpublish_by_headquarters`] / [`LessonMenu::republish_by_headquarters`]。
//! 本部が非公開にしたことと理由は [`HeadquartersUnpublish`]）。本部が非公開にした・公開に戻すたびに、誰がいつどうしたかの記録
//! [`LessonMenuPublicationChange`] を返す。本部が名前で探すときの検索語は [`LessonMenuNameQuery`]。

mod headquarters_unpublish;
mod headquarters_unpublish_reason;
mod lesson_duration;
mod lesson_format;
mod lesson_menu;
mod lesson_menu_content;
mod lesson_menu_description;
mod lesson_menu_id;
mod lesson_menu_name;
mod lesson_menu_name_query;
mod lesson_menu_publication_action;
mod lesson_menu_publication_change;
mod sport;

pub use headquarters_unpublish::HeadquartersUnpublish;
pub use headquarters_unpublish_reason::HeadquartersUnpublishReason;
pub use lesson_duration::LessonDuration;
pub use lesson_format::LessonFormat;
pub use lesson_menu::LessonMenu;
pub use lesson_menu_content::LessonMenuContent;
pub use lesson_menu_description::LessonMenuDescription;
pub use lesson_menu_id::LessonMenuId;
pub use lesson_menu_name::LessonMenuName;
pub use lesson_menu_name_query::LessonMenuNameQuery;
pub use lesson_menu_publication_action::LessonMenuPublicationAction;
pub use lesson_menu_publication_change::LessonMenuPublicationChange;
pub use sport::Sport;
