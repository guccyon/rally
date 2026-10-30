//! catalog: カタログ（specs/context-map.yaml の catalog）。コーチが提供するレッスンのメニュー
//! （glossary の lesson。レッスン種別）を集約 [`LessonMenu`] で表す。
//!
//! コーチとレッスン内容のセットが商品（docs/adr/0009）。メニューを作る・直す・非公開にできるのはそのコーチだけで、
//! スポーツはそのコーチの提供スポーツ（identity の [`crate::identity::Coach`]）のどれか。同期 / 非同期の別は
//! [`crate::lesson::LessonType`] を使う。料金（billing）や予約枠との結びつき（scheduling）は持たない。

mod lesson_duration;
mod lesson_menu;
mod lesson_menu_content;
mod lesson_menu_description;
mod lesson_menu_id;
mod lesson_menu_name;

pub use lesson_duration::LessonDuration;
pub use lesson_menu::LessonMenu;
pub use lesson_menu_content::LessonMenuContent;
pub use lesson_menu_description::LessonMenuDescription;
pub use lesson_menu_id::LessonMenuId;
pub use lesson_menu_name::LessonMenuName;
