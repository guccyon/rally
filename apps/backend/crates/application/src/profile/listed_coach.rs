use domain::{identity::Account, lesson::CoachLessonCount, scheduling::CoachAcceptanceStatus};

/// 本部が見るコーチの一覧（[`super::ListCoaches`]）の 1 件。コーチのアカウントと、scheduling に問い合わせた受付の状態と、
/// 直近 30 日に終わったレッスンの数（同期・非同期の別。domain の [`CoachLessonCount::tally_recent`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedCoach {
    pub account: Account,
    pub acceptance: CoachAcceptanceStatus,
    pub recent_lessons: CoachLessonCount,
}
