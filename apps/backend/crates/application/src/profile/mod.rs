//! profile のユースケースと、他のコンテキストに表示名・紹介（と、本部からのお知らせを受け取る生徒）を渡すアダプタ。
//!
//! データは当面 identity のアカウントの保存先（[`AccountRepository`]。Postgres の `identity` スキーマの表）に
//! あり、それを読み書きする（docs/adr/0009-subdomain-types.md）。
//!
//! [`AccountRepository`]: crate::identity::AccountRepository

mod coach_favorite_lookup;
mod coach_profile;
mod get_coach_profile;
mod list_coaches;
mod profile_announcement_recipient_lookup;
mod profile_coach_candidate_lookup;
mod profile_coach_display_name_lookup;
mod profile_coach_introduction_lookup;
mod profile_student_display_name_lookup;
mod profile_student_sport_lookup;
mod update_coach_profile;
mod update_student_profile;

pub use coach_favorite_lookup::CoachFavoriteLookup;
pub use coach_profile::CoachProfile;
pub use get_coach_profile::{GetCoachProfile, GetCoachProfileCommand};
pub use list_coaches::ListCoaches;
pub use profile_announcement_recipient_lookup::ProfileAnnouncementRecipientLookup;
pub use profile_coach_candidate_lookup::ProfileCoachCandidateLookup;
pub use profile_coach_display_name_lookup::ProfileCoachDisplayNameLookup;
pub use profile_coach_introduction_lookup::ProfileCoachIntroductionLookup;
pub use profile_student_display_name_lookup::ProfileStudentDisplayNameLookup;
pub use profile_student_sport_lookup::ProfileStudentSportLookup;
pub use update_coach_profile::{
    UpdateCoachProfile, UpdateCoachProfileCommand, UpdateCoachProfileError,
};
pub use update_student_profile::{
    UpdateStudentProfile, UpdateStudentProfileCommand, UpdateStudentProfileError,
};
