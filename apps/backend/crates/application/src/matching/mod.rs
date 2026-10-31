//! matching のユースケースとポート。生徒がスポーツでコーチを探す一覧（student-app の `listCoachesForStudent`）。
//!
//! matching は自分の保存先を持たず、他のコンテキストの持ち物をポートで読むだけ。コーチの候補（提供スポーツと表示名）と
//! 生徒のスポーツは profile（[`CoachCandidateLookup`]・[`StudentSportLookup`]。実装は
//! [`crate::profile::ProfileCoachCandidateLookup`]・[`crate::profile::ProfileStudentSportLookup`]）、評価の件数と平均は
//! review（[`CoachReviewSummaryLookup`]。実装は [`crate::review::ReviewCoachReviewSummaryLookup`]）、公開中の
//! レッスンメニューの数は catalog（[`PublishedLessonMenuCountLookup`]。実装は
//! [`crate::catalog::CatalogPublishedLessonMenuCountLookup`]）、受付を止めているコーチ（一覧に出さない）は scheduling
//! （[`SuspendedCoachLookup`]。実装は [`crate::scheduling::SchedulingCoachAcceptanceLookup`]）。誰が呼べるか（生徒だけ）は
//! 境界（interface）が確かめる。

mod coach_candidate;
mod coach_candidate_lookup;
mod coach_listing;
mod coach_review_summary_lookup;
mod list_coaches_for_student;
mod published_lesson_menu_count_lookup;
mod student_sport_lookup;
mod suspended_coach_lookup;

pub use coach_candidate::CoachCandidate;
pub use coach_candidate_lookup::CoachCandidateLookup;
pub use coach_listing::CoachListing;
pub use coach_review_summary_lookup::CoachReviewSummaryLookup;
pub use list_coaches_for_student::{
    ListCoachesForStudent, ListCoachesForStudentCommand, ListCoachesForStudentError,
};
pub use published_lesson_menu_count_lookup::PublishedLessonMenuCountLookup;
pub use student_sport_lookup::StudentSportLookup;
pub use suspended_coach_lookup::SuspendedCoachLookup;
