//! catalog のユースケースとポート。コーチのレッスンメニュー（glossary の lesson「レッスン種別」）の作成・変更と一覧。
//!
//! メニューを作る・直すコーチの提供スポーツは identity のアカウントの保存先（[`AccountRepository`]）から読む。
//! scheduling がメニューを問い合わせるポート（[`crate::scheduling::LessonMenuLookup`]）の実装 [`CatalogLessonMenuLookup`] も置く。
//! 誰が呼べるか（作成・変更・自分の一覧はコーチだけ、公開中の一覧は生徒）は境界（interface）が確かめる。
//!
//! [`AccountRepository`]: crate::identity::AccountRepository

mod catalog_lesson_menu_lookup;
mod create_lesson_menu;
pub mod lesson_menu_id_uuid;
mod list_coach_lesson_menus;
mod list_my_lesson_menus;
mod repository;
mod update_lesson_menu;

pub use catalog_lesson_menu_lookup::CatalogLessonMenuLookup;
pub use create_lesson_menu::{CreateLessonMenu, CreateLessonMenuCommand, CreateLessonMenuError};
pub use list_coach_lesson_menus::ListCoachLessonMenus;
pub use list_my_lesson_menus::ListMyLessonMenus;
pub use repository::LessonMenuRepository;
pub use update_lesson_menu::{UpdateLessonMenu, UpdateLessonMenuCommand, UpdateLessonMenuError};
