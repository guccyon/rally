//! profile のユースケースと、他のコンテキストに表示名・紹介を渡すアダプタ。
//!
//! データは当面 identity のアカウントの保存先（[`AccountRepository`]。Postgres の `identity` スキーマの表）に
//! あり、それを読み書きする（docs/adr/0009-subdomain-types.md）。
//!
//! [`AccountRepository`]: crate::identity::AccountRepository

mod list_coaches;
mod profile_coach_introduction_lookup;
mod profile_student_display_name_lookup;
mod update_coach_profile;

pub use list_coaches::ListCoaches;
pub use profile_coach_introduction_lookup::ProfileCoachIntroductionLookup;
pub use profile_student_display_name_lookup::ProfileStudentDisplayNameLookup;
pub use update_coach_profile::{
    UpdateCoachProfile, UpdateCoachProfileCommand, UpdateCoachProfileError,
};
