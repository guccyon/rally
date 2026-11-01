//! profile のユースケースと、他のコンテキストに表示名・紹介（と、本部からのお知らせを受け取る生徒）を渡すアダプタ。
//!
//! コーチの自己紹介文は profile のポート（[`CoachProfileRepository`]）で読み書きする。表示名・提供スポーツは当面
//! identity のアカウントの保存先（[`AccountRepository`]）から読み、生徒の表示名もそこに書く。どちらのデータも当面は
//! Postgres の `identity` スキーマの表にある（docs/adr/0009-subdomain-types.md）。
//!
//! [`AccountRepository`]: crate::identity::AccountRepository

mod coach_acceptance_lookup;
mod coach_favorite_lookup;
mod coach_overview;
mod coach_profile;
mod coach_profile_repository;
mod coach_recent_lesson_count_lookup;
mod get_coach_bio;
mod get_coach_overview;
mod get_coach_profile;
mod list_coaches;
mod listed_coach;
mod profile_announcement_recipient_lookup;
mod profile_coach_candidate_lookup;
mod profile_coach_display_name_lookup;
mod profile_coach_introduction_lookup;
mod profile_student_display_name_lookup;
mod profile_student_sport_lookup;
mod update_coach_profile;
mod update_student_profile;

pub use coach_acceptance_lookup::CoachAcceptanceLookup;
pub use coach_favorite_lookup::CoachFavoriteLookup;
pub use coach_overview::CoachOverview;
pub use coach_profile::CoachProfile;
pub use coach_profile_repository::CoachProfileRepository;
pub use coach_recent_lesson_count_lookup::CoachRecentLessonCountLookup;
pub use get_coach_bio::GetCoachBio;
pub use get_coach_overview::GetCoachOverview;
pub use get_coach_profile::{GetCoachProfile, GetCoachProfileCommand};
pub use list_coaches::ListCoaches;
pub use listed_coach::ListedCoach;
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
