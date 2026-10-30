//! catalog のユースケースとポート。コーチのレッスンメニュー（glossary の lesson。レッスン種別）の作成・変更と、
//! コーチ本人の一覧・生徒が見る公開中の一覧。
//!
//! メニューのスポーツがコーチの提供スポーツかを確かめるために、コーチを [`CoachLookup`] で読む（コーチの持ち主は
//! identity。実装は [`crate::identity::IdentityCoachLookup`]）。

mod coach_lookup;
mod create_lesson_menu;
pub mod lesson_menu_id_uuid;
mod lesson_menu_listing;
mod list_coach_lesson_menus;
mod list_my_lesson_menus;
mod repository;
mod update_lesson_menu;

pub use coach_lookup::CoachLookup;
pub use create_lesson_menu::{CreateLessonMenu, CreateLessonMenuCommand, CreateLessonMenuError};
pub use list_coach_lesson_menus::ListCoachLessonMenus;
pub use list_my_lesson_menus::ListMyLessonMenus;
pub use repository::LessonMenuRepository;
pub use update_lesson_menu::{UpdateLessonMenu, UpdateLessonMenuCommand, UpdateLessonMenuError};
