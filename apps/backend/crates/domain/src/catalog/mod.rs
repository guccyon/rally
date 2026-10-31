//! catalog: カタログ（specs/context-map.yaml の catalog）。コーチとレッスン内容のセットが商品（docs/adr/0009）。
//!
//! 提供スポーツの種類 [`Sport`]（identity の生徒・コーチもこれを参照する）と、
//! コーチのレッスンメニュー（glossary の lesson「レッスン種別」。スポーツごとにコーチが提供するメニューで、
//! 同期と非同期の別を持つ）を集約 [`LessonMenu`] で持つ。契約の `Lesson`（student-app の完了したレッスン）と
//! [`crate::lesson`]（完了の記録）と区別するため、型の名前は「レッスンメニュー」にした。
//! メニューを作る・直す・非公開にするのはそのコーチだけ（[`LessonMenu::create`] / [`LessonMenu::revise`]）。

mod lesson_duration;
mod lesson_format;
mod lesson_menu;
mod lesson_menu_content;
mod lesson_menu_description;
mod lesson_menu_id;
mod lesson_menu_name;
mod sport;

pub use lesson_duration::LessonDuration;
pub use lesson_format::LessonFormat;
pub use lesson_menu::LessonMenu;
pub use lesson_menu_content::LessonMenuContent;
pub use lesson_menu_description::LessonMenuDescription;
pub use lesson_menu_id::LessonMenuId;
pub use lesson_menu_name::LessonMenuName;
pub use sport::Sport;
