//! identity のユースケースとポート（登録・本人のアカウントの照会・本部が見る生徒の一覧と生徒 1 人の利用の様子・認証）。
//! 表示名・自己紹介文を扱うユースケースは [`crate::profile`]。

pub mod account_id_uuid;
mod get_account;
mod get_student_activity;
mod list_students;
mod register_account;
mod repository;
mod student_activity;
mod student_activity_coach_display_name_lookup;
mod student_booking_activity;
mod student_booking_activity_lookup;
mod student_contract;
mod student_contract_lookup;
mod student_last_booking_lookup;
mod student_lesson_history_lookup;
mod student_overview;
mod token;

pub use get_account::GetAccount;
pub use get_student_activity::{GetStudentActivity, GetStudentActivityError};
pub use list_students::ListStudents;
pub use register_account::{RegisterAccount, RegisterAccountCommand, RegisterAccountError};
pub use repository::{AccountRepository, InsertAccountError};
pub use student_activity::StudentActivity;
pub use student_activity_coach_display_name_lookup::StudentActivityCoachDisplayNameLookup;
pub use student_booking_activity::StudentBookingActivity;
pub use student_booking_activity_lookup::StudentBookingActivityLookup;
pub use student_contract::StudentContract;
pub use student_contract_lookup::StudentContractLookup;
pub use student_last_booking_lookup::StudentLastBookingLookup;
pub use student_lesson_history_lookup::StudentLessonHistoryLookup;
pub use student_overview::StudentOverview;
pub use token::{AccessToken, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier};
