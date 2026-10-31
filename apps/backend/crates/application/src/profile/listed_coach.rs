use domain::{identity::Account, scheduling::CoachAcceptanceStatus};

/// 本部が見るコーチの一覧（[`super::ListCoaches`]）の 1 件。コーチのアカウントと、scheduling に問い合わせた受付の状態。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedCoach {
    pub account: Account,
    pub acceptance: CoachAcceptanceStatus,
}
