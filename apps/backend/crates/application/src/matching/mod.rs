//! matching のユースケースとポート。生徒がスポーツでコーチを探す一覧（student-app の `listCoachesForStudent`）と、
//! 生徒のお気に入りのコーチ（`addFavoriteCoach`・`removeFavoriteCoach`・`listFavoriteCoaches`）。
//!
//! matching の持ち物はお気に入り（[`FavoriteCoachRepository`]。Postgres の `matching.favorite_coaches`）だけで、ほかは
//! 他のコンテキストの持ち物をポートで読む。コーチの候補（提供スポーツと表示名）と
//! 生徒のスポーツは profile（[`CoachCandidateLookup`]・[`StudentSportLookup`]。実装は
//! [`crate::profile::ProfileCoachCandidateLookup`]・[`crate::profile::ProfileStudentSportLookup`]）、評価の件数と平均は
//! review（[`CoachReviewSummaryLookup`]。実装は [`crate::review::ReviewCoachReviewSummaryLookup`]）、公開中の
//! レッスンメニューの数は catalog（[`PublishedLessonMenuCountLookup`]。実装は
//! [`crate::catalog::CatalogPublishedLessonMenuCountLookup`]）。お気に入りに入れる・一覧で添えるコーチの表示名と
//! 提供スポーツは、catalog の `CreateLessonMenu` と同じくアカウントの保存先（[`crate::identity::AccountRepository`]）から
//! 読む。profile のコーチの紹介には、お気に入りかを [`MatchingCoachFavoriteLookup`] で渡す。誰が呼べるか（生徒だけ）は
//! 境界（interface）が確かめる。

mod add_favorite_coach;
mod coach_candidate;
mod coach_candidate_lookup;
mod coach_listing;
mod coach_review_summary_lookup;
mod favorite_coach_listing;
mod favorite_coach_repository;
mod list_coaches_for_student;
mod list_favorite_coaches;
mod matching_coach_favorite_lookup;
mod published_lesson_menu_count_lookup;
mod remove_favorite_coach;
mod student_sport_lookup;

pub use add_favorite_coach::{AddFavoriteCoach, AddFavoriteCoachCommand, AddFavoriteCoachError};
pub use coach_candidate::CoachCandidate;
pub use coach_candidate_lookup::CoachCandidateLookup;
pub use coach_listing::CoachListing;
pub use coach_review_summary_lookup::CoachReviewSummaryLookup;
pub use favorite_coach_listing::FavoriteCoachListing;
pub use favorite_coach_repository::FavoriteCoachRepository;
pub use list_coaches_for_student::{
    ListCoachesForStudent, ListCoachesForStudentCommand, ListCoachesForStudentError,
};
pub use list_favorite_coaches::ListFavoriteCoaches;
pub use matching_coach_favorite_lookup::MatchingCoachFavoriteLookup;
pub use published_lesson_menu_count_lookup::PublishedLessonMenuCountLookup;
pub use remove_favorite_coach::RemoveFavoriteCoach;
pub use student_sport_lookup::StudentSportLookup;
